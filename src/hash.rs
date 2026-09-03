//! BLAKE3 hashing utilities for file and string content.
//!
//! Provides functions for computing BLAKE3 hashes of files, byte slices,
//! and strings, as well as hex encoding.

use blake3::Hash;
use std::io::Read;

/// Compute the BLAKE3 hash of a file's contents.
///
/// Reads the file in 64 KiB chunks to avoid loading large files entirely
/// into memory.
pub fn hash_file(path: &std::path::Path) -> Result<Hash, crate::error::NgdarError> {
    let mut file = std::fs::File::open(path).map_err(|e| {
        crate::error::NgdarError::Hash(format!("Cannot open {}: {}", path.display(), e))
    })?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| {
            crate::error::NgdarError::Hash(format!("Cannot read {}: {}", path.display(), e))
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize())
}

/// Compute the BLAKE3 hash of a string.
pub fn hash_string(data: &str) -> Hash {
    blake3::hash(data.as_bytes())
}

/// Convert a BLAKE3 hash to a hex string.
pub fn hash_to_hex(hash: &Hash) -> String {
    hash.to_hex().to_string()
}
