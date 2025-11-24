use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    // Start server thread
    thread::spawn(|| {
        if let Err(e) = run_server() {
            eprintln!("Server error: {}", e);
        }
    });
    
    // Give server time to start
    thread::sleep(Duration::from_secs(1));
    
    // Test client
    test_client()?;
    
    Ok(())
}

fn run_server() -> std::io::Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;
    println!("Server listening...");
    
    let (mut stream, addr) = listener.accept()?;
    println!("Connection from: {}", addr);
    
    // Read client public key (32 bytes)
    let mut client_key = [0u8; 32];
    stream.read_exact(&mut client_key)?;
    println!("Read client key: {:?}", &client_key[..8]);
    
    // Send server public key (32 bytes)
    let server_key = [42u8; 32];
    stream.write_all(&server_key)?;
    println!("Sent server key");
    
    // Read encrypted command
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    println!("Reading {} bytes of encrypted data", len);
    
    let mut encrypted = vec![0u8; len];
    stream.read_exact(&mut encrypted)?;
    println!("Read encrypted data: {:?}", &encrypted[..16]);
    
    // Send response
    let response = b"test response";
    let len = response.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(response)?;
    
    Ok(())
}

fn test_client() -> std::io::Result<()> {
    let mut stream = TcpStream::connect("127.0.0.1:8080")?;
    println!("Connected to server");
    
    // Send client public key
    let client_key = [123u8; 32];
    stream.write_all(&client_key)?;
    println!("Sent client key");
    
    // Read server public key
    let mut server_key = [0u8; 32];
    stream.read_exact(&mut server_key)?;
    println!("Read server key: {:?}", &server_key[..8]);
    
    // Send encrypted command
    let command = b"TEST";
    let len = command.len() as u32;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(command)?;
    println!("Sent command");
    
    // Read response
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;
    
    let mut response = vec![0u8; len];
    stream.read_exact(&mut response)?;
    println!("Response: {}", String::from_utf8_lossy(&response));
    
    Ok(())
}