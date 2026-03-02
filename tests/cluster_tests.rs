use redix::run_node;
use redix::raft::Command;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::info;

#[tokio::test]
async fn test_cluster_replication() -> anyhow::Result<()> {
    // Setup tracing for the test
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();

    // 1. Setup 3 nodes with unique ports
    let peer_count = 3;
    let (notify_tx, mut notify_rx) = mpsc::unbounded_channel();

    // Node 0
    let tx0 = notify_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_node(
            0, 3, 
            "127.0.0.1:9100".to_string(), 
            "127.0.0.1:9101".to_string(), 
            vec![(1, "127.0.0.1:9110".to_string()), (2, "127.0.0.1:9120".to_string())],
            Some(tx0)
        ).await {
            eprintln!("Node 0 failed: {}", e);
        }
    });
    sleep(Duration::from_millis(100)).await;

    // Node 1
    let tx1 = notify_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_node(
            1, 3, 
            "127.0.0.1:9110".to_string(), 
            "127.0.0.1:9111".to_string(), 
            vec![(0, "127.0.0.1:9100".to_string()), (2, "127.0.0.1:9120".to_string())],
            Some(tx1)
        ).await {
            eprintln!("Node 1 failed: {}", e);
        }
    });
    sleep(Duration::from_millis(100)).await;

    // Node 2
    let tx2 = notify_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_node(
            2, 3, 
            "127.0.0.1:9120".to_string(), 
            "127.0.0.1:9121".to_string(), 
            vec![(0, "127.0.0.1:9100".to_string()), (1, "127.0.0.1:9110".to_string())],
            Some(tx2)
        ).await {
            eprintln!("Node 2 failed: {}", e);
        }
    });

    // 2. Wait for election
    sleep(Duration::from_millis(3000)).await;

    // 3. Issue a SET command to a node (find the leader)
    let mut success = false;
    for port in [9101, 9111, 9121] {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            stream.write_all(b"SET foo bar\n").await?;
            let mut buf = [0; 128];
            if let Ok(n) = stream.read(&mut buf).await {
                let resp = String::from_utf8_lossy(&buf[..n]);
                if resp.contains("OK") {
                    success = true;
                    break;
                }
            }
        }
    }
    
    assert!(success, "Failed to issue SET command to cluster leader");

    // 4. Verify replication
    let mut applied_nodes = std::collections::HashSet::new();
    let timeout = sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    info!("Waiting for replication on 3 nodes...");
    loop {
        tokio::select! {
            Some((node_id, cmd)) = notify_rx.recv() => {
                if let Command::Set(k, v) = cmd {
                    if k == "foo" && v == "bar" {
                        applied_nodes.insert(node_id);
                        info!("Node {} applied foo=bar (Total: {})", node_id, applied_nodes.len());
                        if applied_nodes.len() == 3 { break; }
                    }
                }
            }
            _ = &mut timeout => {
                info!("Timed out waiting for replication. Applied nodes: {:?}", applied_nodes);
                break;
            }
        }
    }

    assert_eq!(applied_nodes.len(), 3, "Replication failed: expected 3 nodes to apply command, but only nodes {:?} did", applied_nodes);

    Ok(())
}
