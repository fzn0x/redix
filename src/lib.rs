pub mod persistence;
pub mod raft;
pub mod kv_store;
pub mod network;

use dashmap::DashMap;
use tracing::info;
use std::time::Duration;
use tokio::sync::mpsc;
use crate::raft::{RaftNode, Command, LogEntry, RaftState};
pub use crate::kv_store::KvStore;
use crate::network::{NetworkManager, Message};
use crate::network::redis::RedisServer;

pub async fn run_node(
    node_id: u64,
    peer_count: usize,
    raft_addr: String,
    redis_addr: String,
    peers: Vec<(u64, String)>,
    mut notify_applied_tx: Option<mpsc::UnboundedSender<(u64, Command)>>,
) -> anyhow::Result<()> {
    let mut raft = RaftNode::new(node_id, peer_count);
    let kv = KvStore::new();
    let storage = persistence::PersistenceManager::new(node_id);

    // --- RECOVERY ---
    if let Some(snapshot) = storage.load_snapshot::<DashMap<String, String>>()? {
        info!("Node {}: Loaded RDB snapshot", node_id);
        kv.restore_state(snapshot);
    }
    
    let meta = storage.load_metadata()?.unwrap_or(persistence::RaftMetadata { current_term: 0, voted_for: None });
    let logs = storage.read_log::<LogEntry>()?;
    info!("Node {}: Recovered term={}, voted_for={:?}, {} logs", node_id, meta.current_term, meta.voted_for, logs.len());
    
    raft.restore(meta.current_term, meta.voted_for, logs);
    // Note: Replaying logs usually happens by advancing commit_index if we know it, 
    // but in this simple impl, we'll assume logs in AOF were committed or we rely on Raft to sync.
    // For a real impl, we'd persist commit_index too.
    // Let's just replay all current logs to the KV store for simplicity in this version.
    for entry in &raft.log {
        match entry.command {
            Command::Set(ref k, ref v) => kv.set(k.clone(), v.clone()),
            Command::Delete(ref k) => kv.delete(k),
            Command::Keys => {},
            Command::Flush => kv.clear(),
        }
    }
    // ----------------

    let (tx, mut rx) = mpsc::channel(100);
    NetworkManager::start_server(raft_addr, tx).await?;
    
    let mut net_manager = NetworkManager::new();
    for (id, addr) in peers {
        net_manager.add_peer(id, addr);
    }

    let mut votes = 0;
    let mut interval = tokio::time::interval(Duration::from_millis(10));
    let mut snapshot_interval = tokio::time::interval(Duration::from_secs(30));

    let (client_tx, mut client_rx) = mpsc::channel(100);
    let redis_server = RedisServer::new(redis_addr);
    redis_server.start(client_tx).await?;

    let mut pending_queries: std::collections::HashMap<String, mpsc::Sender<String>> = std::collections::HashMap::new();

    let mut last_meta = persistence::RaftMetadata { current_term: raft.current_term, voted_for: raft.voted_for };

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let (vote_req, append_reqs) = raft.tick(peer_count);
                
                if let Some(req) = vote_req {
                    info!("Node {} started election for term {}", node_id, req.term);
                    votes = 1; 
                    if votes > peer_count / 2 {
                        raft.state = RaftState::Leader;
                        info!("Node {} became Leader for term {} (Single-node cluster)", node_id, raft.current_term);
                    } else {
                        for peer_id in 0..peer_count as u64 {
                            if peer_id == node_id { continue; }
                            let _ = net_manager.send_message(peer_id, Message::RequestVote { 
                                args: req.clone(), 
                                from_id: node_id 
                            }).await;
                        }
                    }
                }

                for (peer, req) in append_reqs {
                    let _ = net_manager.send_message(peer, Message::AppendEntries { 
                        args: req, 
                        from_id: node_id 
                    }).await;
                }
            }
            _ = snapshot_interval.tick() => {
                if raft.state == RaftState::Leader {
                    info!("Node {}: Saving RDB snapshot...", node_id);
                    storage.save_snapshot(&kv.get_state())?;
                    // In a production system, we'd truncate logs here.
                }
            }
            message_result = rx.recv() => {
                if let Some((_sender_id, msg)) = message_result {
                    match msg {
                        Message::RequestVote { args, from_id } => {
                            let reply = raft.handle_request_vote(args);
                            let _ = net_manager.send_message(from_id, Message::RequestVoteReply { 
                                reply, 
                                from_id: node_id 
                            }).await;
                        }
                        Message::RequestVoteReply { reply, from_id: _ } => {
                            raft.handle_vote_reply(reply, peer_count, &mut votes);
                        }
                        Message::AppendEntries { args, from_id } => {
                            let old_log_len = raft.log.len();
                            let reply = raft.handle_append_entries(args);
                            if raft.log.len() > old_log_len {
                                // Persist new entries
                                for i in old_log_len..raft.log.len() {
                                    storage.append_log(&raft.log[i])?;
                                }
                            }
                            let _ = net_manager.send_message(from_id, Message::AppendEntriesReply { 
                                reply, 
                                from_id: node_id 
                            }).await;
                        }
                        Message::AppendEntriesReply { reply, from_id } => {
                            raft.handle_append_entries_reply(from_id, reply);
                        }
                    }
                }
            }
            client_result = client_rx.recv() => {
                if let Some((cmd, resp_tx)) = client_result {
                    match cmd {
                        Command::Set(ref k, ref v) if v == "QUERY_GET" => {
                            let res = kv.get(k).unwrap_or_else(|| "nil".to_string());
                            resp_tx.send(res).await.ok();
                        }
                        Command::Set(k, v) => {
                            if raft.state == RaftState::Leader {
                                let entry = LogEntry { term: raft.current_term, command: Command::Set(k.clone(), v.clone()) };
                                storage.append_log(&entry)?;
                                raft.log.push(entry);
                                
                                let last_index = raft.log.len() as u64;
                                pending_queries.insert(format!("set_{}", last_index), resp_tx);
                                
                                if peer_count == 1 {
                                    raft.commit_index = last_index;
                                }
                            } else {
                                resp_tx.send("ERR not leader".to_string()).await.ok();
                            }
                        }
                        Command::Delete(k) => {
                            if raft.state == RaftState::Leader {
                                let entry = LogEntry { term: raft.current_term, command: Command::Delete(k.clone()) };
                                storage.append_log(&entry)?;
                                raft.log.push(entry);

                                let last_index = raft.log.len() as u64;
                                pending_queries.insert(format!("del_{}", last_index), resp_tx);
                                
                                if peer_count == 1 {
                                    raft.commit_index = last_index;
                                }
                            } else {
                                resp_tx.send("ERR not leader".to_string()).await.ok();
                            }
                        }
                        Command::Keys => {
                            let keys = kv.keys();
                            let res = if keys.is_empty() {
                                "(empty list or set)".to_string()
                            } else {
                                keys.join("\n")
                            };
                            resp_tx.send(res).await.ok();
                        }
                        Command::Flush => {
                            if raft.state == RaftState::Leader {
                                let entry = LogEntry { term: raft.current_term, command: Command::Flush };
                                storage.append_log(&entry)?;
                                raft.log.push(entry);

                                let last_index = raft.log.len() as u64;
                                pending_queries.insert(format!("flush_{}", last_index), resp_tx);
                                
                                if peer_count == 1 {
                                    raft.commit_index = last_index;
                                }
                            } else {
                                resp_tx.send("ERR not leader".to_string()).await.ok();
                            }
                        }
                    }
                }
            }
        }

        // Persist metadata if it changed
        if raft.current_term != last_meta.current_term || raft.voted_for != last_meta.voted_for {
            last_meta = persistence::RaftMetadata { current_term: raft.current_term, voted_for: raft.voted_for };
            storage.save_metadata(&last_meta)?;
        }

        for command in raft.get_unapplied_commands() {
            let applied_index = raft.last_applied;
            match command {
                Command::Set(k, v) => {
                    info!("Node {}: Applying SET {}={}", node_id, k, v);
                    kv.set(k.clone(), v.clone());
                    if let Some(ref mut n_tx) = notify_applied_tx {
                        n_tx.send((node_id, Command::Set(k.clone(), v.clone()))).ok();
                    }
                    if let Some(tx) = pending_queries.remove(&format!("set_{}", applied_index)) {
                        tx.send("OK".to_string()).await.ok();
                    }
                }
                Command::Delete(k) => {
                    info!("Node {}: Applying DELETE {}", node_id, k);
                    kv.delete(&k);
                    if let Some(ref mut n_tx) = notify_applied_tx {
                        n_tx.send((node_id, Command::Delete(k.clone()))).ok();
                    }
                    if let Some(tx) = pending_queries.remove(&format!("del_{}", applied_index)) {
                        tx.send("1".to_string()).await.ok();
                    }
                }
                Command::Keys => {} // Handled synchronously by leader
                Command::Flush => {
                    info!("Node {}: Applying FLUSHALL", node_id);
                    kv.clear();
                    if let Some(ref mut n_tx) = notify_applied_tx {
                        n_tx.send((node_id, Command::Flush)).ok();
                    }
                    if let Some(tx) = pending_queries.remove(&format!("flush_{}", applied_index)) {
                        tx.send("OK".to_string()).await.ok();
                    }
                }
            }
        }
    }
}
