use std::error::Error;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use walkdir::WalkDir;

const STORAGE_ROOT: &str = "gilma_storage";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Ensure the storage root directory exists
    let storage_path = PathBuf::from(STORAGE_ROOT);
    if !storage_path.exists() {
        fs::create_dir_all(&storage_path).await?;
        println!("Created storage directory: {:?}", storage_path);
    } else {
        println!("Using storage directory: {:?}", storage_path);
    }

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Gilma server listening on 127.0.0.1:8080");

    loop {
        let (stream, addr) = listener.accept().await?;

        tokio::spawn(async move {
            println!("Accepted connection from: {}", addr);

            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            let mut storage_base_path = PathBuf::from(STORAGE_ROOT);
            let mut in_push_dir_mode = false;

            loop {
                line.clear();
                let bytes_read = match reader.read_line(&mut line).await {
                    Ok(0) => {
                        println!("Connection closed from {}", addr);
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("[{}] Error reading line: {}", addr, e);
                        break;
                    }
                };
                
                if bytes_read == 0 && line.is_empty() {
                    break; // Proper stream closure
                }

                let trimmed_line = line.trim();
                
                if trimmed_line.starts_with("PUSH_DIR ") {
                    if in_push_dir_mode {
                        eprintln!("[{}] Protocol error: PUSH_DIR command received while already in PUSH_DIR mode.", addr);
                        break;
                    }
                    let dir_name = trimmed_line["PUSH_DIR ".len()..].trim();
                    if dir_name.is_empty() {
                        eprintln!("[{}] Protocol error: PUSH_DIR command missing directory name.", addr);
                        break;
                    }
                    storage_base_path = PathBuf::from(STORAGE_ROOT).join(dir_name);
                    if let Err(e) = fs::create_dir_all(&storage_base_path).await {
                        eprintln!("[{}] Failed to create base directory {:?}: {}", addr, storage_base_path, e);
                        break;
                    }
                    println!("[{}] Starting PUSH_DIR for: {:?}", addr, storage_base_path);
                    in_push_dir_mode = true;
                } else if trimmed_line.starts_with("FILE ") {
                    if !in_push_dir_mode {
                        eprintln!("[{}] Protocol error: FILE command received outside of PUSH_DIR mode.", addr);
                        break;
                    }
                    let parts: Vec<&str> = trimmed_line["FILE ".len()..].splitn(2, ' ').collect();
                    if parts.len() != 2 {
                        eprintln!("[{}] Protocol error: FILE command malformed: {}", addr, trimmed_line);
                        break;
                    }
                    let relative_path_str = parts[0];
                    let content_len: usize = match parts[1].parse() {
                        Ok(len) => len,
                        Err(e) => {
                            eprintln!("[{}] Protocol error: Invalid content length '{}': {}", addr, parts[1], e);
                            break;
                        }
                    };

                    let dest_path = storage_base_path.join(relative_path_str);
                    if let Some(parent_dir) = dest_path.parent() {
                        if let Err(e) = fs::create_dir_all(parent_dir).await {
                            eprintln!("[{}] Failed to create parent directories for {:?}: {}", addr, dest_path, e);
                            break;
                        }
                    }

                    let mut file_content = vec![0; content_len];
                    if let Err(e) = reader.read_exact(&mut file_content).await {
                        eprintln!("[{}] Error reading file content for {:?}: {}", addr, dest_path, e);
                        break;
                    }

                    if let Err(e) = fs::write(&dest_path, &file_content).await {
                        eprintln!("[{}] Failed to write file {:?}: {}", addr, dest_path, e);
                    } else {
                        println!("[{}] Successfully wrote file: {:?}", addr, dest_path);
                    }
                } else if trimmed_line == "END_PUSH" {
                    if !in_push_dir_mode {
                        eprintln!("[{}] Protocol error: END_PUSH command received outside of PUSH_DIR mode.", addr);
                        break;
                    }
                    println!("[{}] PUSH_DIR complete for: {:?}", addr, storage_base_path);
                    break;
                } else if trimmed_line == "LIST" {
                    if in_push_dir_mode {
                        eprintln!("[{}] Protocol error: LIST command received during PUSH_DIR mode.", addr);
                        break;
                    }
                    // List all folders in storage
                    let storage_path = PathBuf::from(STORAGE_ROOT);
                    if let Ok(mut entries) = fs::read_dir(&storage_path).await {
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            if let Ok(metadata) = entry.metadata().await {
                                if metadata.is_dir() {
                                    if let Some(name) = entry.file_name().to_str() {
                                        let _ = tokio::io::AsyncWriteExt::write_all(
                                            &mut reader.get_mut(),
                                            format!("{}\n", name).as_bytes()
                                        ).await;
                                    }
                                }
                            }
                        }
                    }
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut reader.get_mut(),
                        b"END_LIST\n"
                    ).await;
                    break;
                } else if trimmed_line.starts_with("CHECK ") {
                    if in_push_dir_mode {
                        eprintln!("[{}] Protocol error: CHECK command received during PUSH_DIR mode.", addr);
                        break;
                    }
                    let folder_name = trimmed_line["CHECK ".len()..].trim();
                    if folder_name.is_empty() {
                        eprintln!("[{}] Protocol error: CHECK command missing folder name.", addr);
                        break;
                    }
                    
                    let folder_path = PathBuf::from(STORAGE_ROOT).join(folder_name);
                    if folder_path.exists() && folder_path.is_dir() {
                        // Send file list with timestamps
                        for entry in WalkDir::new(&folder_path).into_iter().filter_map(|e| e.ok()) {
                            if entry.file_type().is_file() {
                                let path = entry.path();
                                let relative_path = path.strip_prefix(&folder_path).unwrap();
                                let relative_path_str = relative_path.to_str().unwrap_or_default();
                                
                                if let Ok(metadata) = fs::metadata(path).await {
                                    if let Ok(modified) = metadata.modified() {
                                        let timestamp = modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                                        let _ = tokio::io::AsyncWriteExt::write_all(
                                            &mut reader.get_mut(),
                                            format!("{} {}\n", relative_path_str, timestamp).as_bytes()
                                        ).await;
                                    }
                                }
                            }
                        }
                    }
                    
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut reader.get_mut(),
                        b"END_CHECK\n"
                    ).await;
                    break;
                } else if trimmed_line.starts_with("SYNC_DIR ") {
                    if in_push_dir_mode {
                        eprintln!("[{}] Protocol error: SYNC_DIR command received during PUSH_DIR mode.", addr);
                        break;
                    }
                    let dir_name = trimmed_line["SYNC_DIR ".len()..].trim();
                    if dir_name.is_empty() {
                        eprintln!("[{}] Protocol error: SYNC_DIR command missing directory name.", addr);
                        break;
                    }
                    storage_base_path = PathBuf::from(STORAGE_ROOT).join(dir_name);
                    if let Err(e) = fs::create_dir_all(&storage_base_path).await {
                        eprintln!("[{}] Failed to create base directory {:?}: {}", addr, storage_base_path, e);
                        break;
                    }
                    println!("[{}] Starting SYNC_DIR for: {:?}", addr, storage_base_path);
                    in_push_dir_mode = true;
                } else if trimmed_line == "END_SYNC" {
                    if !in_push_dir_mode {
                        eprintln!("[{}] Protocol error: END_SYNC command received outside of SYNC_DIR mode.", addr);
                        break;
                    }
                    println!("[{}] SYNC_DIR complete for: {:?}", addr, storage_base_path);
                    break;
                } else if trimmed_line.starts_with("PULL ") {
                    if in_push_dir_mode {
                        eprintln!("[{}] Protocol error: PULL command received during PUSH_DIR mode.", addr);
                        break;
                    }
                    let folder_name = trimmed_line["PULL ".len()..].trim();
                    if folder_name.is_empty() {
                        eprintln!("[{}] Protocol error: PULL command missing folder name.", addr);
                        break;
                    }
                    
                    let folder_path = PathBuf::from(STORAGE_ROOT).join(folder_name);
                    if !folder_path.exists() || !folder_path.is_dir() {
                        eprintln!("[{}] Folder not found: {:?}", addr, folder_path);
                        break;
                    }
                    
                    // Send all files in the folder
                    for entry in WalkDir::new(&folder_path).into_iter().filter_map(|e| e.ok()) {
                        if entry.file_type().is_file() {
                            let path = entry.path();
                            let relative_path = path.strip_prefix(&folder_path).unwrap();
                            let relative_path_str = relative_path.to_str().unwrap_or_default();
                            
                            let content = match fs::read(path).await {
                                Ok(content) => content,
                                Err(e) => {
                                    eprintln!("[{}] Error reading file {:?}: {}", addr, path, e);
                                    continue;
                                }
                            };
                            
                            let file_cmd = format!("FILE {} {}\n", relative_path_str, content.len());
                            if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                                &mut reader.get_mut(),
                                file_cmd.as_bytes()
                            ).await {
                                eprintln!("[{}] Error sending file command: {}", addr, e);
                                break;
                            }
                            
                            if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                                &mut reader.get_mut(),
                                &content
                            ).await {
                                eprintln!("[{}] Error sending file content: {}", addr, e);
                                break;
                            }
                            
                            println!("[{}] Sent file: {} ({} bytes)", addr, relative_path_str, content.len());
                        }
                    }
                    
                    let _ = tokio::io::AsyncWriteExt::write_all(
                        &mut reader.get_mut(),
                        b"END_PULL\n"
                    ).await;
                    println!("[{}] PULL complete for: {}", addr, folder_name);
                    break;
                } else if !trimmed_line.is_empty() {
                    eprintln!("[{}] Protocol error: Unrecognized command or out of sequence: '{}'", addr, trimmed_line);
                    break;
                }
            }
            println!("Connection handler for {} finished.", addr);
        });
    }
}
