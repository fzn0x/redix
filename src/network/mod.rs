pub mod redis;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::raft::{AppendEntriesArgs, AppendEntriesReply, RequestVoteArgs, RequestVoteReply};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    RequestVote { args: RequestVoteArgs, from_id: u64 },
    RequestVoteReply { reply: RequestVoteReply, from_id: u64 },
    AppendEntries { args: AppendEntriesArgs, from_id: u64 },
    AppendEntriesReply { reply: AppendEntriesReply, from_id: u64 },
}

pub struct NetworkManager {
    peers: HashMap<u64, String>, // ID -> Address
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, id: u64, addr: String) {
        self.peers.insert(id, addr);
    }

    pub async fn send_message(&self, peer_id: u64, msg: Message) -> anyhow::Result<()> {
        if let Some(addr) = self.peers.get(&peer_id) {
            let mut stream = TcpStream::connect(addr).await?;
            let data = bincode::serialize(&msg)?;
            stream.write_u32(data.len() as u32).await?;
            stream.write_all(&data).await?;
        }
        Ok(())
    }

    pub async fn start_server(addr: String, tx: mpsc::Sender<(u64, Message)>) -> anyhow::Result<()> {
        let listener = TcpListener::bind(addr).await?;
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        loop {
                            let len = match stream.read_u32().await {
                                Ok(l) => l,
                                Err(_) => break,
                            };
                            let mut buf = vec![0; len as usize];
                            if stream.read_exact(&mut buf).await.is_err() {
                                break;
                            }
                            if let Ok(msg) = bincode::deserialize::<Message>(&buf) {
                                let from_id = match &msg {
                                    Message::RequestVote { from_id, .. } => *from_id,
                                    Message::RequestVoteReply { from_id, .. } => *from_id,
                                    Message::AppendEntries { from_id, .. } => *from_id,
                                    Message::AppendEntriesReply { from_id, .. } => *from_id,
                                };
                                tx.send((from_id, msg)).await.ok(); 
                            }
                        }
                    });
                }
            }
        });
        Ok(())
    }
}
