use std::net::TcpStream;
use std::io::{self, Write, Read};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::env;
use std::time::Instant;

fn get_sync_stats(path_to_scan: &Path) -> (u64, u64) {
    let mut total_size = 0;
    let mut file_count = 0;
    for entry in fs::read_dir(path_to_scan).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            let (size, count) = get_sync_stats(&path);
            total_size += size;
            file_count += count;
        } else {
            if let Ok(metadata) = fs::metadata(&path) {
                total_size += metadata.len();
                file_count += 1;
            }
        }
    }
    (total_size, file_count)
}

fn send_directory(
    stream: &mut TcpStream,
    path_to_scan: &Path,
    base_path: &Path,
    overall_bytes_sent: &mut u64,
    total_size: u64,
    files_sent_count: &mut u64,
    total_files: u64,
    overall_start_time: Instant,
) {
    for entry in fs::read_dir(path_to_scan).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.is_dir() {
            send_directory(
                stream,
                &path,
                base_path,
                overall_bytes_sent,
                total_size,
                files_sent_count,
                total_files,
                overall_start_time,
            );
        } else {
            let relative_path = path.strip_prefix(base_path).unwrap();
            let file_path_str = relative_path.to_str().unwrap();

            let metadata = fs::metadata(&path).unwrap();
            let file_size = metadata.len();

            let new_file_command = format!("NEW_FILE {} {}\n", file_path_str, file_size);
            stream.write_all(new_file_command.as_bytes()).unwrap();

            if file_size > 0 {
                let mut file = File::open(&path).unwrap();
                const CHUNK_SIZE: usize = 8192; // 8 KB
                let mut buffer = vec![0; CHUNK_SIZE];

                loop {
                    let bytes_read = file.read(&mut buffer).unwrap();
                    if bytes_read == 0 {
                        break;
                    }

                    stream.write_all(&buffer[..bytes_read]).unwrap();
                    *overall_bytes_sent += bytes_read as u64;

                    let elapsed = overall_start_time.elapsed();
                    let speed_mbps = if elapsed.as_secs_f64() > 0.0 {
                        (*overall_bytes_sent as f64 / 1_000_000.0) / elapsed.as_secs_f64() * 8.0
                    } else {
                        0.0
                    };

                    let progress = if total_size > 0 {
                        *overall_bytes_sent as f64 / total_size as f64
                    } else {
                        1.0
                    };
                    let progress_percent = progress * 100.0;

                    const BAR_WIDTH: usize = 40;
                    let filled_width = (progress * BAR_WIDTH as f64) as usize;
                    let empty_width = BAR_WIDTH - filled_width;

                    let bar = format!("[{}{}]", "█".repeat(filled_width), "-".repeat(empty_width));

                    print!(
                        "\rSyncing: {} {:.2}% ({}/{} files, {:.2} Mbps) ",
                        bar, progress_percent, *files_sent_count + 1, total_files, speed_mbps
                    );
                    io::stdout().flush().unwrap();
                }
            }
            *files_sent_count += 1;
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: {} <command>", args[0]);
        println!("Available commands: vechuko");
        return;
    }
    let command = &args[1];

    let path_to_send: PathBuf = match command.as_str() {
        "vechuko" => env::current_dir().unwrap(),
        _ => {
            println!("Unknown command: {}", command);
            return;
        }
    };

    if !path_to_send.is_dir() {
        println!("Error: '{}' is not a directory.", path_to_send.display());
        return;
    }

    match TcpStream::connect("127.0.0.1:8888") {
        Ok(mut stream) => {
            println!("Connected to server.");

            println!("Calculating sync stats for '{}'...", path_to_send.display());
            let (total_size, total_files) = get_sync_stats(&path_to_send);
            println!("Ready to send {} files ({} bytes).", total_files, total_size);

            let dir_name = path_to_send.file_name().unwrap().to_str().unwrap();
            let start_command = format!("START_SESSION {}\n", dir_name);
            stream.write_all(start_command.as_bytes()).unwrap();
            println!("Starting session for directory: {}", dir_name);

            let mut overall_bytes_sent = 0;
            let mut files_sent_count = 0;
            let overall_start_time = Instant::now();

            send_directory(
                &mut stream,
                &path_to_send,
                &path_to_send,
                &mut overall_bytes_sent,
                total_size,
                &mut files_sent_count,
                total_files,
                overall_start_time,
            );

            // Final progress update to show 100%
            let bar = format!("[{}]", "█".repeat(40));
            let elapsed = overall_start_time.elapsed();
            let speed_mbps = if elapsed.as_secs_f64() > 0.0 {
                (total_size as f64 / 1_000_000.0) / elapsed.as_secs_f64() * 8.0
            } else {
                0.0
            };
            print!(
                "\rSyncing: {} 100.00% ({}/{} files, {:.2} Mbps) ",
                bar, total_files, total_files, speed_mbps
            );
            io::stdout().flush().unwrap();

            stream.write_all(b"END_SESSION\n").unwrap();
            println!("\nSession ended.");
        }
        Err(e) => {
            println!("Failed to connect: {}", e);
        }
    }
}
