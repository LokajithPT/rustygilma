use std::net::TcpStream;
use std::io::{self, Write, Read};
use std::fs::{self, File};
use std::path::Path;
use std::env;
use std::time::Instant;

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

            let metadata = fs::metadata(&path).unwrap();
            let file_size = metadata.len();

            if file_size == 0 {
                // Don't show a progress bar for empty files
                let new_file_command = format!("NEW_FILE {} {}\n", file_path_str, file_size);
                stream.write_all(new_file_command.as_bytes()).unwrap();
                println!("Sending {}: [████████████████████████████████████████] 100.00% (0.00 Mbps)", file_path_str);
                continue;
            }

            let new_file_command = format!("NEW_FILE {} {}\n", file_path_str, file_size);
            stream.write_all(new_file_command.as_bytes()).unwrap();

            let mut file = File::open(&path).unwrap();
            let mut total_bytes_sent = 0;
            let start_time = Instant::now();

            const CHUNK_SIZE: usize = 8192; // 8 KB
            let mut buffer = vec![0; CHUNK_SIZE];

            loop {
                let bytes_read = file.read(&mut buffer).unwrap();
                if bytes_read == 0 {
                    break;
                }

                stream.write_all(&buffer[..bytes_read]).unwrap();
                total_bytes_sent += bytes_read as u64;

                let elapsed = start_time.elapsed();
                let speed_mbps = if elapsed.as_secs_f64() > 0.0 {
                    (total_bytes_sent as f64 / 1_000_000.0) / elapsed.as_secs_f64() * 8.0
                } else {
                    0.0
                };

                let progress = total_bytes_sent as f64 / file_size as f64;
                let progress_percent = progress * 100.0;

                const BAR_WIDTH: usize = 40;
                let filled_width = (progress * BAR_WIDTH as f64) as usize;
                let empty_width = BAR_WIDTH - filled_width;

                let bar = format!("[{}{}]", "█".repeat(filled_width), "-".repeat(empty_width));

                print!("\rSending {}: {} {:.2}% ({:.2} Mbps) ", file_path_str, bar, progress_percent, speed_mbps);
                io::stdout().flush().unwrap();
            }
            println!(); // Newline after the progress bar
        }
    }
}
