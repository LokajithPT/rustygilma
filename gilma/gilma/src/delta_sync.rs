use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct ChunkInfo {
    pub index: usize,
    pub checksum: String,
    pub size: usize,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct FileDelta {
    pub chunks_to_add: Vec<(usize, Vec<u8>)>,
    pub chunks_to_remove: Vec<usize>,
    pub chunks_to_keep: Vec<ChunkInfo>,
}

pub struct DeltaSync;

impl DeltaSync {
    pub const CHUNK_SIZE: usize = 4096; // 4KB chunks for delta calculation
    
    pub async fn calculate_file_delta(
        local_path: &Path,
        remote_chunks: &[ChunkInfo],
    ) -> Result<FileDelta, Box<dyn std::error::Error + Send + Sync>> {
        let local_content = fs::read(local_path).await?;
        let local_chunks = Self::chunk_file(&local_content);
        
        let mut chunks_to_add = Vec::new();
        let mut chunks_to_remove = Vec::new();
        let mut chunks_to_keep = Vec::new();
        
        // Create hashmap of remote chunks for quick lookup
        let remote_chunk_map: HashMap<usize, &ChunkInfo> = remote_chunks
            .iter()
            .map(|chunk| (chunk.index, chunk))
            .collect();
        
        // Find chunks to keep and remove
        for (index, local_chunk) in local_chunks.iter().enumerate() {
            if let Some(remote_chunk) = remote_chunk_map.get(&index) {
                if local_chunk.checksum == remote_chunk.checksum {
                    chunks_to_keep.push(local_chunk.clone());
                } else {
                    chunks_to_remove.push(index);
                    chunks_to_add.push((index, local_chunk.data.clone()));
                }
            } else {
                chunks_to_add.push((index, local_chunk.data.clone()));
            }
        }
        
        // Find chunks that exist remotely but not locally (to be removed)
        for remote_chunk in remote_chunks {
            if !chunks_to_keep.iter().any(|c| c.index == remote_chunk.index) &&
               !chunks_to_remove.contains(&remote_chunk.index) {
                chunks_to_remove.push(remote_chunk.index);
            }
        }
        
        debug!("Delta calculated: {} chunks to keep, {} to add, {} to remove", 
               chunks_to_keep.len(), chunks_to_add.len(), chunks_to_remove.len());
        
        Ok(FileDelta {
            chunks_to_add,
            chunks_to_remove,
            chunks_to_keep,
        })
    }
    
    pub fn chunk_file(content: &[u8]) -> Vec<ChunkInfo> {
        let mut chunks = Vec::new();
        
        for (index, chunk_data) in content.chunks(Self::CHUNK_SIZE).enumerate() {
            let checksum = blake3::hash(chunk_data).to_string();
            chunks.push(ChunkInfo {
                index,
                checksum,
                size: chunk_data.len(),
                data: chunk_data.to_vec(),
            });
        }
        
        chunks
    }
    
    pub fn apply_delta(base_content: &[u8], delta: &FileDelta) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut result = Vec::new();
        let base_chunks: HashMap<usize, Vec<u8>> = base_content
            .chunks(Self::CHUNK_SIZE)
            .enumerate()
            .map(|(i, chunk)| (i, chunk.to_vec()))
            .collect();
        
        // Reconstruct file from chunks
        let mut max_index = 0;
        
        // Find the maximum index
        for chunk in &delta.chunks_to_keep {
            max_index = max_index.max(chunk.index);
        }
        for (index, _) in &delta.chunks_to_add {
            max_index = max_index.max(*index);
        }
        
        // Build the file chunk by chunk
        for index in 0..=max_index {
            // Check if we need to add this chunk
            if let Some((_, data)) = delta.chunks_to_add.iter().find(|(i, _)| *i == index) {
                result.extend_from_slice(data);
            }
            // Check if we should keep this chunk from base
            else if delta.chunks_to_keep.iter().any(|c| c.index == index) {
                if let Some(data) = base_chunks.get(&index) {
                    result.extend_from_slice(data);
                }
            }
            // Skip chunks that are marked for removal
            else if delta.chunks_to_remove.contains(&index) {
                continue;
            }
        }
        
        Ok(result)
    }
    
    pub fn serialize_chunks(chunks: &[ChunkInfo]) -> String {
        chunks
            .iter()
            .map(|chunk| format!("{}:{}:{}", chunk.index, chunk.checksum, chunk.size))
            .collect::<Vec<_>>()
            .join(",")
    }
    
    pub fn deserialize_chunks(data: &str) -> Result<Vec<ChunkInfo>, Box<dyn std::error::Error>> {
        let mut chunks = Vec::new();
        
        for chunk_str in data.split(',') {
            if chunk_str.is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = chunk_str.split(':').collect();
            if parts.len() != 3 {
                continue;
            }
            
            let index = parts[0].parse::<usize>()?;
            let checksum = parts[1].to_string();
            let size = parts[2].parse::<usize>()?;
            
            chunks.push(ChunkInfo {
                index,
                checksum,
                size,
                data: Vec::new(), // Data not included in serialization
            });
        }
        
        Ok(chunks)
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_delta_sync() {
        let original = b"Hello, world! This is a test file for delta sync.";
        let modified = b"Hello, world! This is a modified test file for delta sync.";
        
        let original_chunks = DeltaSync::chunk_file(original);
        let modified_chunks = DeltaSync::chunk_file(modified);
        
        // Simulate remote chunks (original)
        let remote_chunks: Vec<ChunkInfo> = original_chunks
            .into_iter()
            .map(|chunk| ChunkInfo {
                index: chunk.index,
                checksum: chunk.checksum.clone(),
                size: chunk.size,
                data: Vec::new(), // Clear data for remote representation
            })
            .collect();
        
        // Calculate delta
        let delta = DeltaSync::calculate_file_delta(
            Path::new("test_modified.txt"),
            &remote_chunks,
        ).await.unwrap();
        
        // Apply delta to reconstruct modified file
        let reconstructed = DeltaSync::apply_delta(original, &delta).unwrap();
        
        assert_eq!(reconstructed, modified);
    }
}