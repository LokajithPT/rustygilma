//wassup nigesh 
use clap::{Parser, Subcommand};
use walkdir::WalkDir;
use std::env::current_dir;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt, AsyncReadExt};
use tokio::net::TcpStream;
use colored::*;

#[derive(Parser, Debug)]
#[command(name = "gilma", version, about = "Gilma - Awesome File Sync Tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Push entire directory to server
    Vechuko,
    /// List all folders in storage
    Kaami,
    /// Pull a specific folder from storage
    Vangiko {
        /// Name of the folder to pull
        folder_name: String,
    },
    /// Sync only changed files with server
    Sync,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Vechuko => { 
            if let Err(e) = push_directory().await {
                eprintln!("{}", format!("Error pushing directory: {}", e).red().bold());
            }
        }
        Commands::Kaami => {
            if let Err(e) = list_folders().await {
                eprintln!("{}", format!("Error listing folders: {}", e).red().bold());
            }
        }
        Commands::Vangiko { folder_name } => {
            if let Err(e) = pull_folder(folder_name).await {
                eprintln!("{}", format!("Error pulling folder: {}", e).red().bold());
            }
        }
        Commands::Sync => {
            if let Err(e) = sync_directory().await {
                eprintln!("{}", format!("Error syncing directory: {}", e).red().bold());
            }
        }
    }
    
    Ok(())
}

async fn sync_directory() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir = std::env::current_dir()?;
    let dir_name = current_dir.file_name().unwrap_or_default().to_str().unwrap_or_default();

    // First connection: check server files
    let mut stream = TcpStream::connect("100.104.132.24:8080").await?;
    println!("{}", "Connected to server for sync check".green().bold());

    let check_cmd = format!("CHECK {}\n", dir_name);
    stream.write_all(check_cmd.as_bytes()).await?;
    
    let mut reader = tokio::io::BufReader::new(stream);
    let mut line = String::new();
    let mut server_files = std::collections::HashMap::new();
    
    // Read server file list with timestamps
    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed == "END_CHECK" {
            break;
        }
        if !trimmed.is_empty() {
            if let Some((file_path, timestamp)) = trimmed.split_once(' ') {
                server_files.insert(file_path.to_string(), timestamp.parse::<u64>().unwrap_or(0));
            }
        }
        line.clear();
    }
    drop(reader); // Close the first connection
    
    // Now walk local directory and compare
    let mut files_to_send = Vec::new();
    
    for entry in WalkDir::new(&current_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let relative_path = path.strip_prefix(&current_dir).unwrap();
            let relative_path_str = relative_path.to_str().unwrap_or_default();

            if relative_path_str.is_empty() { continue; }
            if relative_path.starts_with("target") || relative_path.starts_with(".git") || 
               relative_path.starts_with(".venv") || relative_path.starts_with("node_modules") ||
               relative_path.starts_with("dist") || relative_path.starts_with("build") ||
               relative_path.starts_with("__pycache__") || relative_path.starts_with(".pytest_cache") ||
               relative_path.starts_with(".mypy_cache") || relative_path.starts_with("coverage") { continue; }

            let metadata = std::fs::metadata(path)?;
            let local_modified = metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs();
            
            let should_send = match server_files.get(relative_path_str) {
                Some(server_timestamp) => local_modified > *server_timestamp,
                None => true, // New file
            };
            
            if should_send {
                files_to_send.push((relative_path_str.to_string(), path.to_path_buf()));
            }
        }
    }
    
    if files_to_send.is_empty() {
        println!("{}", "No files need syncing. Everything is up to date.".green().bold());
        return Ok(());
    }
    
    println!("{}", format!("Syncing {} changed files...", files_to_send.len()).yellow().bold());
    
    // Second connection: send changed files
    let mut stream = TcpStream::connect("100.104.132.24:8080").await?;
    
    // Send SYNC_DIR command
    let sync_cmd = format!("SYNC_DIR {}\n", dir_name);
    stream.write_all(sync_cmd.as_bytes()).await?;
    
    let mut total_bytes = 0;
    
    // Send only changed files
    let files_count = files_to_send.len();
    for (relative_path, full_path) in files_to_send {
        let content = fs::read(&full_path).await?;
        
        let file_cmd = format!("FILE {} {}\n", relative_path, content.len());
        stream.write_all(file_cmd.as_bytes()).await?;
        stream.write_all(&content).await?;
        
        println!("  {} {} ({} bytes)", "SYNC".cyan(), relative_path.bright_white(), content.len().to_string().dimmed());
        total_bytes += content.len();
    }
    
    // Send END_SYNC command
    stream.write_all(b"END_SYNC\n").await?;
    println!("{}", "Sync complete!".green().bold());
    println!("{}", format!("Summary: {} files, {} bytes synced", files_count, total_bytes).magenta());
    
    Ok(())
}

async fn push_directory() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("100.104.132.24:8080").await?;
    println!("{}", "Connected to server".green().bold());

    let current_dir = std::env::current_dir()?;
    let dir_name = current_dir.file_name().unwrap_or_default().to_str().unwrap_or_default();

    println!("{}", format!("Pushing directory: {}", dir_name).cyan().bold());

    // 1. Send PUSH_DIR command
    let push_dir_cmd = format!("PUSH_DIR {}\n", dir_name);
    stream.write_all(push_dir_cmd.as_bytes()).await?;
    println!("{}", "Started directory push".yellow());

    let mut file_count = 0;
    let mut total_bytes = 0;

    // 2. Walk directory and send each file
    for entry in WalkDir::new(&current_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            // Get path relative to the directory we are pushing
            let relative_path = path.strip_prefix(&current_dir).unwrap();
            let relative_path_str = relative_path.to_str().unwrap_or_default();

            if relative_path_str.is_empty() { continue; }
            if relative_path.starts_with("target") || relative_path.starts_with(".git") || 
               relative_path.starts_with(".venv") || relative_path.starts_with("node_modules") ||
               relative_path.starts_with("dist") || relative_path.starts_with("build") ||
               relative_path.starts_with("__pycache__") || relative_path.starts_with(".pytest_cache") ||
               relative_path.starts_with(".mypy_cache") || relative_path.starts_with("coverage") { continue; }

            let content = fs::read(path).await?;
            
            // Send FILE command
            let file_cmd = format!("FILE {} {}\n", relative_path_str, content.len());
            stream.write_all(file_cmd.as_bytes()).await?;

            // Send file content
            stream.write_all(&content).await?;

            file_count += 1;
            total_bytes += content.len();
            println!("  {} {} ({} bytes)", "->".blue(), relative_path_str.bright_white(), content.len().to_string().dimmed());
        }
    }

    // 3. Send END_PUSH command
    stream.write_all(b"END_PUSH\n").await?;
    println!("{}", "Directory push complete!".green().bold());
    println!("{}", format!("Summary: {} files, {} bytes transferred", file_count, total_bytes).magenta());

    Ok(())
}

async fn list_folders() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("100.104.132.24:8080").await?;
    println!("{}", "Connected to server".green().bold());
    
    // Send LIST command
    stream.write_all(b"LIST\n").await?;
    
    // Read response
    let mut reader = tokio::io::BufReader::new(stream);
    let mut line = String::new();
    
    println!("{}", "\nFolders in storage:".cyan().bold());
    println!("{}", "─".repeat(40).dimmed());
    
    let mut folder_count = 0;
    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();
        if trimmed == "END_LIST" {
            break;
        }
        if !trimmed.is_empty() {
            println!("  {} {}", "[DIR]".yellow(), trimmed.bright_white());
            folder_count += 1;
        }
        line.clear();
    }
    
    println!("{}", "─".repeat(40).dimmed());
    println!("{}", format!("Total folders: {}", folder_count).magenta());
    
    Ok(())
}

async fn pull_folder(folder_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect("100.104.132.24:8080").await?;
    println!("{}", "Connected to server".green().bold());
    
    // Send PULL command
    let pull_cmd = format!("PULL {}\n", folder_name);
    stream.write_all(pull_cmd.as_bytes()).await?;
    println!("{}", format!("Requesting folder: {}", folder_name).cyan().bold());
    
    let mut reader = tokio::io::BufReader::new(stream);
    let mut line = String::new();
    
    // Create local directory
    fs::create_dir_all(folder_name).await?;
    
    let mut file_count = 0;
    let mut total_bytes = 0;
    
    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }
        
        let trimmed_line = line.trim();
        
        if trimmed_line.starts_with("FILE ") {
            let parts: Vec<&str> = trimmed_line["FILE ".len()..].splitn(2, ' ').collect();
            if parts.len() != 2 {
                eprintln!("{}", format!("Malformed FILE command: {}", trimmed_line).red());
                break;
            }
            
            let relative_path = parts[0];
            let content_len: usize = parts[1].parse()?;
            
            // Skip target and .git directories
            if relative_path.starts_with("target") || relative_path.starts_with(".git") || 
               relative_path.starts_with(".venv") || relative_path.starts_with("node_modules") ||
               relative_path.starts_with("dist") || relative_path.starts_with("build") ||
               relative_path.starts_with("__pycache__") || relative_path.starts_with(".pytest_cache") ||
               relative_path.starts_with(".mypy_cache") || relative_path.starts_with("coverage") {
                // Still need to read the content to skip it
                let mut file_content = vec![0; content_len];
                reader.read_exact(&mut file_content).await?;
                continue;
            }
            
            let local_path = PathBuf::from(folder_name).join(relative_path);
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            
            let mut file_content = vec![0; content_len];
            reader.read_exact(&mut file_content).await?;
            
            fs::write(&local_path, &file_content).await?;
            println!("  {} {} ({} bytes)", "<-".blue(), relative_path.bright_white(), content_len.to_string().dimmed());
            file_count += 1;
            total_bytes += content_len;
        } else if trimmed_line == "END_PULL" {
            println!("{}", "Pull complete!".green().bold());
            println!("{}", format!("Summary: {} files, {} bytes received", file_count, total_bytes).magenta());
            break;
        }
    }
    
    Ok(())
}
