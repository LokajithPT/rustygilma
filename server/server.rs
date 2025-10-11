use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write, Read};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

fn handle_client(stream: TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut session_dir: Option<PathBuf> = None;

    loop {
        let mut command = String::new();
        match reader.read_line(&mut command) {
            Ok(0) => {
                println!("Client disconnected.");
                break;
            }
            Ok(_) => {
                let command = command.trim();
                let parts: Vec<&str> = command.split_whitespace().collect();

                if parts.is_empty() {
                    continue;
                }

                match parts[0] {
                    "START_SESSION" => {
                        if parts.len() > 1 {
                            let dir_name = parts[1];
                            let new_session_dir = Path::new("br1").join(dir_name);
                            fs::create_dir_all(&new_session_dir).expect("Failed to create session directory");
                            println!("Started session for: {}", dir_name);
                            session_dir = Some(new_session_dir);
                        }
                    }
                    "NEW_FILE" => {
                        if let Some(ref base_dir) = session_dir {
                            if parts.len() > 2 {
                                let relative_path = parts[1];
                                if let Ok(file_size) = parts[2].parse::<u64>() {
                                    let file_path = base_dir.join(relative_path);
                                    if let Some(parent_dir) = file_path.parent() {
                                        fs::create_dir_all(parent_dir).expect("Failed to create parent directories");
                                    }

                                    let mut file_content = Vec::with_capacity(file_size as usize);
                                    
                                    let buffered = reader.buffer();
                                    let to_read_from_buffer = std::cmp::min(buffered.len() as u64, file_size) as usize;
                                    file_content.extend_from_slice(&buffered[..to_read_from_buffer]);
                                    reader.consume(to_read_from_buffer);

                                    let remaining_size = file_size as usize - file_content.len();
                                    if remaining_size > 0 {
                                        let mut remaining_buffer = vec![0; remaining_size];
                                        reader.get_mut().read_exact(&mut remaining_buffer).expect("Failed to read remaining file content");
                                        file_content.extend_from_slice(&remaining_buffer);
                                    }

                                    let mut file = File::create(&file_path).expect("Failed to create file");
                                    file.write_all(&file_content).expect("Failed to write to file");
                                    println!("Received file: {} ({} bytes)", relative_path, file_size);
                                }
                            }
                        }
                    }
                    "END_SESSION" => {
                        println!("Ended session.");
                        session_dir = None;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                eprintln!("Error reading from stream: {}", e);
                break;
            }
        }
    }
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8888").unwrap();
    println!("Server listening on 127.0.0.1:8888");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_client(stream);
                });
            }
            Err(e) => {
                eprintln!("Unable to connect: {}", e);
            }
        }
    }
}
