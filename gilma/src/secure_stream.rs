use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use crate::crypto::CryptoEngine;
use std::io;

pub struct SecureStream {
    stream: TcpStream,
    crypto: CryptoEngine,
}

impl SecureStream {
    pub fn new(stream: TcpStream, crypto: CryptoEngine) -> Self {
        Self { stream, crypto }
    }
    
    pub async fn write_line(&mut self, line: &str) -> io::Result<()> {
        let data = format!("{}\n", line);
        let encrypted = self.crypto.encrypt(data.as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        // Send length prefix then encrypted data
        let len = encrypted.len() as u32;
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(&encrypted).await?;
        self.stream.flush().await?;
        Ok(())
    }
    
    pub async fn write_bytes(&mut self, data: &[u8]) -> io::Result<()> {
        let encrypted = self.crypto.encrypt(data)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let len = encrypted.len() as u32;
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(&encrypted).await?;
        self.stream.flush().await?;
        Ok(())
    }
    
    pub async fn read_line(&mut self) -> io::Result<String> {
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        eprintln!("DEBUG: Reading {} bytes of encrypted data", len);
        
        let mut encrypted_data = vec![0u8; len];
        self.stream.read_exact(&mut encrypted_data).await?;
        eprintln!("DEBUG: Read {} encrypted bytes", encrypted_data.len());
        
        let decrypted = self.crypto.decrypt(&encrypted_data)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        let line = String::from_utf8_lossy(&decrypted);
        eprintln!("DEBUG: Decrypted line: '{}'", line);
        Ok(line.trim_end_matches('\n').to_string())
    }
    
    pub async fn read_bytes(&mut self) -> io::Result<Vec<u8>> {
        let mut len_bytes = [0u8; 4];
        self.stream.read_exact(&mut len_bytes).await?;
        let len = u32::from_le_bytes(len_bytes) as usize;
        
        let mut encrypted_data = vec![0u8; len];
        self.stream.read_exact(&mut encrypted_data).await?;
        
        let decrypted = self.crypto.decrypt(&encrypted_data)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        Ok(decrypted)
    }
    
    pub fn get_crypto(&self) -> &CryptoEngine {
        &self.crypto
    }
    
    pub fn get_crypto_mut(&mut self) -> &mut CryptoEngine {
        &mut self.crypto
    }
}