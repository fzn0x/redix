use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use crate::raft::Command;

pub struct RedisServer {
    addr: String,
}

impl RedisServer {
    pub fn new(addr: String) -> Self {
        Self { addr }
    }

    pub async fn start(&self, tx: mpsc::Sender<(Command, mpsc::Sender<String>)>) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.addr).await?;
        
        tokio::spawn(async move {
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let mut buf = [0; 512];
                        loop {
                            let n = match stream.read(&mut buf).await {
                                Ok(0) | Err(_) => break,
                                Ok(n) => n,
                            };
                            
                            let input = String::from_utf8_lossy(&buf[..n]);
                            let parts: Vec<&str> = input.trim().split_whitespace().collect();
                            
                            if parts.is_empty() { continue; }
                            
                            match parts[0].to_uppercase().as_str() {
                                "SET" if parts.len() == 3 => {
                                    let (resp_tx, mut resp_rx) = mpsc::channel(1);
                                    let cmd = Command::Set(parts[1].to_string(), parts[2].to_string());
                                    tx.send((cmd, resp_tx)).await.ok();
                                    if let Some(res) = resp_rx.recv().await {
                                        stream.write_all(format!("+{}\r\n", res).as_bytes()).await.ok();
                                    }
                                }
                                "GET" if parts.len() == 2 => {
                                    let (resp_tx, mut resp_rx) = mpsc::channel(1);
                                    tx.send((Command::Set(parts[1].to_string(), "QUERY_GET".to_string()), resp_tx)).await.ok();
                                    if let Some(res) = resp_rx.recv().await {
                                        stream.write_all(format!("${}\r\n{}\r\n", res.len(), res).as_bytes()).await.ok();
                                    }
                                }
                                "DEL" if parts.len() == 2 => {
                                    let (resp_tx, mut resp_rx) = mpsc::channel(1);
                                    let cmd = Command::Delete(parts[1].to_string());
                                    tx.send((cmd, resp_tx)).await.ok();
                                    if let Some(res) = resp_rx.recv().await {
                                        // RESP integer: :1\r\n
                                        stream.write_all(format!(":{}\r\n", res).as_bytes()).await.ok();
                                    }
                                }
                                "KEYS" => {
                                    let (resp_tx, mut resp_rx) = mpsc::channel(1);
                                    tx.send((Command::Keys, resp_tx)).await.ok();
                                    if let Some(res) = resp_rx.recv().await {
                                        stream.write_all(format!("${}\r\n{}\r\n", res.len(), res).as_bytes()).await.ok();
                                    }
                                }
                                "FLUSHALL" => {
                                    let (resp_tx, mut resp_rx) = mpsc::channel(1);
                                    tx.send((Command::Flush, resp_tx)).await.ok();
                                    if let Some(res) = resp_rx.recv().await {
                                        stream.write_all(format!("+{}\r\n", res).as_bytes()).await.ok();
                                    }
                                }
                                _ => {
                                    stream.write_all(b"-ERR unknown command\r\n").await.ok();
                                }
                            }
                        }
                    });
                }
            }
        });
        Ok(())
    }
}
