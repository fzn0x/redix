use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Node ID
    #[arg(short = 'i', long)]
    id: u64,

    /// Number of peers in the cluster
    #[arg(short = 'c', long, default_value_t = 3)]
    peers_count: usize,

    /// Address for Raft internal communication
    #[arg(short = 'a', long, default_value = "127.0.0.1:8080")]
    raft_addr: String,

    /// Address for Redis client communication
    #[arg(short = 'r', long, default_value = "127.0.0.1:6379")]
    redis_addr: String,

    /// Peer addresses in format id=addr (can be specified multiple times)
    #[arg(short = 'p', long)]
    peer: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let args = Args::parse();
    
    info!("Starting Redix node {}...", args.id);
    info!("Raft Address: {}", args.raft_addr);
    info!("Redis Address: {}", args.redis_addr);

    let mut peers = Vec::new();
    for p in args.peer {
        let parts: Vec<&str> = p.split('=').collect();
        if parts.len() == 2 {
            let id = parts[0].parse::<u64>()?;
            let addr = parts[1].to_string();
            peers.push((id, addr));
        }
    }

    redix::run_node(
        args.id, 
        args.peers_count, 
        args.raft_addr, 
        args.redis_addr, 
        peers, 
        None
    ).await?;

    Ok(())
}
