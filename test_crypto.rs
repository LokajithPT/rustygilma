// Simple test to verify encryption works
use std::process::Command;

fn main() {
    println!("Testing encrypted communication...");
    
    // Start server in background
    let server = Command::new("cargo")
        .args(&["run", "--bin", "gilma-server"])
        .current_dir("gilma-server")
        .spawn()
        .expect("Failed to start server");
    
    std::thread::sleep(std::time::Duration::from_secs(2));
    
    // Test client
    let client = Command::new("cargo")
        .args(&["run", "--bin", "gilma", "--", "kaami"])
        .current_dir("gilma")
        .output()
        .expect("Failed to run client");
    
    println!("Client stdout: {}", String::from_utf8_lossy(&client.stdout));
    println!("Client stderr: {}", String::from_utf8_lossy(&client.stderr));
    
    // Kill server
    server.kill().expect("Failed to kill server");
}