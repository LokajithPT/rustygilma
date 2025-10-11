use std::net::TcpStream;
use std::io::{Write, Read};
use std::fs::{self, File};
use std::path::Path;
use std::env;

fn main() {
    match TcpStream::connect("127.0.0.1:8888") {
        Ok(mut stream) => {
            println!("Connected to server.");

            let current_path = env::current_dir().unwrap();
            let dir_name = current_path.file_name().unwrap().to_str().unwrap();
            
            let start_command = format!("START_SESSION {}\n", dir_name);
            stream.write_all(start_command.as_bytes()).unwrap();
            println!("Sent session start for directory: {}", dir_name);

            send_directory(&mut stream, &current_path, &current_path);

            stream.write_all(b"END_SESSION\n").unwrap();
            println!("Session ended.");
        }
        Err(e) => {
            println!("Failed to connect: {}", e);
        }
    }
}

fn send_directory(stream: &mut TcpStream, path_to_scan: &Path, base_path: &Path) {
    for entry in fs::read_dir(path_to_scan).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            send_directory(stream, &path, base_path);
        } else {
            let relative_path = path.strip_prefix(base_path).unwrap();
            let file_path_str = relative_path.to_str().unwrap();

            // Get file metadata to find its size
            let metadata = fs::metadata(&path).unwrap();
            let file_size = metadata.len();

            // Send the NEW_FILE command with the file size
            let new_file_command = format!("NEW_FILE {} {}\n", file_path_str, file_size);
            stream.write_all(new_file_command.as_bytes()).unwrap();

            // Send the raw file content
            let mut file = File::open(&path).unwrap();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();
            stream.write_all(&buffer).unwrap();
            
            println!("Sent file: {} ({} bytes)", file_path_str, file_size);
        }
    }
}
