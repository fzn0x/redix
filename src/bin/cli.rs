use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Server address to connect to
    #[arg(short, long, default_value = "127.0.0.1:6380")]
    addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("Connecting to Redix at {}...", args.addr);

    let mut rl = DefaultEditor::new()?;

    loop {
        let readline = rl.readline("redix> ");
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                rl.add_history_entry(trimmed)?;

                if trimmed.to_uppercase() == "EXIT" || trimmed.to_uppercase() == "QUIT" {
                    break;
                }

                if trimmed.to_uppercase() == "HELP" {
                    println!("Available commands:");
                    println!("  SET <key> <value> - Set a key to a value");
                    println!("  GET <key>         - Get the value of a key");
                    println!("  DEL <key>         - Delete a key");
                    println!("  KEYS *            - List all keys");
                    println!("  FLUSHALL          - Delete all keys");
                    println!("  EXIT / QUIT       - Exit the CLI");
                    continue;
                }

                // Try to send to server
                if let Err(e) = send_command(&args.addr, trimmed).await {
                    println!("Error: {}", e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("EOF");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    Ok(())
}

async fn send_command(addr: &str, line: &str) -> anyhow::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_all(format!("{}\n", line).as_bytes()).await?;

    let mut buf = [0; 512];
    let n = stream.read(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf[..n]);
    
    // Basic RESP Parsing for cleaner CLI output
    if response.starts_with('+') {
        // Simple String: +OK\r\n -> OK
        println!("{}", response[1..].trim());
    } else if response.starts_with('$') {
        // Bulk String: $3\r\nbar\r\n -> bar
        let lines: Vec<&str> = response.split("\r\n").collect();
        if lines.len() >= 2 {
            println!("{}", lines[1]);
        } else {
            println!("{}", response.trim());
        }
    } else if response.starts_with(':') {
        // Integer: :1000\r\n -> 1000
        println!("{}", response[1..].trim());
    } else if response.starts_with('-') {
        // Error: -ERR ... -> ERR ...
        println!("{}", response[1..].trim());
    } else {
        print!("{}", response);
    }
    
    Ok(())
}
