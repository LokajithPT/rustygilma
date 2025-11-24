use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use aes_gcm::aead::{Aead, OsRng, rand_core::RngCore};
use sha2::{Sha256, Digest};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct CryptoEngine {
    cipher: Aes256Gcm,
    current_key: [u8; 32],
    key_rotation_counter: u64,
    message_counter: u64,
    last_rotation: u64,
    rotation_interval: u64, // seconds
}

impl CryptoEngine {
    pub fn new(initial_key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(initial_key)
            .expect("Invalid key length");
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
            
        Self {
            cipher,
            current_key: *initial_key,
            key_rotation_counter: 0,
            message_counter: 0,
            last_rotation: now,
            rotation_interval: 300, // 5 minutes
        }
    }
    
    pub fn rotate_key(&mut self) -> [u8; 32] {
        self.key_rotation_counter += 1;
        let mut hasher = Sha256::new();
        
        // Hash current key + counter + timestamp
        hasher.update(self.current_key);
        hasher.update(self.key_rotation_counter.to_le_bytes());
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        hasher.update(now.to_le_bytes());
        
        let new_key = hasher.finalize();
        let key_array: [u8; 32] = new_key.into();
        
        self.cipher = Aes256Gcm::new_from_slice(&key_array)
            .expect("Invalid key length");
        self.current_key = key_array;
        self.last_rotation = now;
        
        key_array
    }
    
    pub fn should_rotate_key(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.last_rotation >= self.rotation_interval
    }
    
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        if self.should_rotate_key() {
            self.rotate_key();
        }
        
        self.message_counter += 1;
        
        // Generate nonce from message counter
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[..8].copy_from_slice(&self.message_counter.to_le_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        // Return: nonce + ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    pub fn decrypt(&mut self, encrypted_data: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted_data.len() < 12 {
            return Err("Invalid encrypted data length".to_string());
        }
        
        let nonce = Nonce::from_slice(&encrypted_data[..12]);
        let ciphertext = &encrypted_data[12..];
        
        let plaintext = self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed: {}", e))?;
        Ok(plaintext)
    }
    
    pub fn get_current_key(&self) -> [u8; 32] {
        self.current_key
    }
}

pub struct KeyExchange {
    private_key: [u8; 32],
    public_key: [u8; 32],
}

impl KeyExchange {
    pub fn new() -> Self {
        // Simple Diffie-Hellman-like key exchange
        // In production, use proper crypto libraries
        let mut private_key = [0u8; 32];
        let mut public_key = [0u8; 32];
        
        OsRng.fill_bytes(&mut private_key);
        
        // Simple "public key" derivation (replace with real DH in production)
        let mut hasher = Sha256::new();
        hasher.update(private_key);
        hasher.update(b"gilma-public-key-salt");
        let hash = hasher.finalize();
        public_key.copy_from_slice(&hash[..32]);
        
        Self { private_key, public_key }
    }
    
    pub fn get_public_key(&self) -> [u8; 32] {
        self.public_key
    }
    
    pub fn derive_shared_key(&self, peer_public_key: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.private_key);
        hasher.update(peer_public_key);
        hasher.update(b"gilma-shared-key-salt");
        let hash = hasher.finalize();
        
        let mut shared_key = [0u8; 32];
        shared_key.copy_from_slice(&hash[..32]);
        shared_key
    }
}