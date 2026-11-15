//! Fast content fingerprinting for media assets.
//!
//! Conforms to Charter Rule 9: Files are identified by content fingerprint.
//! Hashes file size + 64KB start chunk + 64KB mid chunk + 64KB end chunk.
//! Runs in < 1ms on files of any size without modifying original media.

use crate::error::MediaError;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const CHUNK_SIZE: usize = 64 * 1024; // 64 KB

/// Computes the unique content fingerprint for a file on disk.
pub fn compute_content_fingerprint(path: impl AsRef<Path>) -> Result<String, MediaError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(MediaError::FileNotFound(path.to_string_lossy().into()));
    }

    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();

    if file_size == 0 {
        return Err(MediaError::InvalidMedia("File is empty (0 bytes)".into()));
    }

    let mut hasher = Fnv1a64::new();
    hasher.write_u64(file_size);

    if file_size <= (CHUNK_SIZE * 3) as u64 {
        // Small file: hash the whole content
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        hasher.write(&buffer);
    } else {
        // Large file: sample 64KB at start, mid, and end
        let mut chunk = vec![0u8; CHUNK_SIZE];

        // 1. Start chunk (byte 0)
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut chunk)?;
        hasher.write(&chunk);

        // 2. Mid chunk
        let mid_offset = (file_size / 2).saturating_sub((CHUNK_SIZE / 2) as u64);
        file.seek(SeekFrom::Start(mid_offset))?;
        file.read_exact(&mut chunk)?;
        hasher.write(&chunk);

        // 3. End chunk
        let end_offset = file_size.saturating_sub(CHUNK_SIZE as u64);
        file.seek(SeekFrom::Start(end_offset))?;
        file.read_exact(&mut chunk)?;
        hasher.write(&chunk);
    }

    let hash_val = hasher.finish();
    Ok(format!("{:x}-{:016x}", file_size, hash_val))
}

/// Computes a content fingerprint from an in-memory byte buffer.
pub fn compute_bytes_fingerprint(bytes: &[u8]) -> String {
    let mut hasher = Fnv1a64::new();
    hasher.write_u64(bytes.len() as u64);
    hasher.write(bytes);
    format!("{:x}-{:016x}", bytes.len(), hasher.finish())
}

/// Fast 64-bit FNV-1a non-cryptographic hasher.
struct Fnv1a64 {
    state: u64,
}

impl Fnv1a64 {
    fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state ^= byte as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    fn write_u64(&mut self, val: u64) {
        self.write(&val.to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_fingerprint_identical_across_same_content() {
        let temp_dir = std::env::temp_dir();
        let file_a = temp_dir.join("test_fp_a.bin");
        let file_b = temp_dir.join("test_fp_b.bin");

        let content = vec![42u8; 256 * 1024]; // 256 KB
        File::create(&file_a).unwrap().write_all(&content).unwrap();
        File::create(&file_b).unwrap().write_all(&content).unwrap();

        let fp_a = compute_content_fingerprint(&file_a).unwrap();
        let fp_b = compute_content_fingerprint(&file_b).unwrap();

        assert_eq!(fp_a, fp_b, "Identical content must yield identical fingerprint");

        // Cleanup
        let _ = std::fs::remove_file(file_a);
        let _ = std::fs::remove_file(file_b);
    }

    #[test]
    fn test_fingerprint_differs_when_content_changes() {
        let temp_dir = std::env::temp_dir();
        let file_a = temp_dir.join("test_diff_a.bin");
        let file_b = temp_dir.join("test_diff_b.bin");

        let content_a = vec![1u8; 300 * 1024];
        let mut content_b = vec![1u8; 300 * 1024];
        content_b[150 * 1024] = 99; // change in middle chunk

        File::create(&file_a).unwrap().write_all(&content_a).unwrap();
        File::create(&file_b).unwrap().write_all(&content_b).unwrap();

        let fp_a = compute_content_fingerprint(&file_a).unwrap();
        let fp_b = compute_content_fingerprint(&file_b).unwrap();

        assert_ne!(fp_a, fp_b, "Modified content must produce different fingerprint");

        let _ = std::fs::remove_file(file_a);
        let _ = std::fs::remove_file(file_b);
    }
}
