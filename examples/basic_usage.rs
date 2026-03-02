use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "127.0.0.1:6380";
    println!("Connecting to Redix at {}...", addr);
    
    let mut stream = TcpStream::connect(addr).await?;
    println!("Connected!\n");

    // 1. SET key
    println!("> SET example_key example_value");
    send_command(&mut stream, "SET example_key example_value").await?;
    let response = read_response(&mut stream).await?;
    println!("Response: {}\n", response);

    // 2. GET key
    println!("> GET example_key");
    send_command(&mut stream, "GET example_key").await?;
    let response = read_response(&mut stream).await?;
    println!("Response: {}\n", response);

    // 3. DEL key
    println!("> DEL example_key");
    send_command(&mut stream, "DEL example_key").await?;
    let response = read_response(&mut stream).await?;
    println!("Response: {}\n", response);

    // 4. GET key again
    println!("> GET example_key");
    send_command(&mut stream, "GET example_key").await?;
    let response = read_response(&mut stream).await?;
    println!("Response: {}\n", response);

    println!("Example complete.");
    Ok(())
}

async fn send_command(stream: &mut TcpStream, cmd: &str) -> Result<()> {
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;
    Ok(())
}

async fn read_response(stream: &mut TcpStream) -> Result<String> {
    let mut buffer = [0u8; 1024];
    let n = stream.read(&mut buffer).await?;
    let response = String::from_utf8_lossy(&buffer[..n]).to_string();
    
    // Simple cleaning of common RESP responses for the example output
    let clean_res = if response.starts_with('+') {
        response[1..].trim().to_string()
    } else if response.starts_with(':') {
        response[1..].trim().to_string()
    } else if response.starts_with('$') {
        let lines: Vec<&str> = response.split("\r\n").collect();
        if lines.len() >= 2 {
            lines[1].to_string()
        } else {
            response.trim().to_string()
        }
    } else {
        response.trim().to_string()
    };

    Ok(clean_res)
}
