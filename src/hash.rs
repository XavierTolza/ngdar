use blake3::Hash;
use std::io::Read;

/// Compute the BLAKE3 hash of a file's contents.
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

/// Compute the BLAKE3 hash of a byte slice.
pub fn _hash_bytes(data: &[u8]) -> Hash {
    blake3::hash(data)
}

/// Compute the BLAKE3 hash of a string.
pub fn hash_string(data: &str) -> Hash {
    blake3::hash(data.as_bytes())
}

/// Convert a BLAKE3 hash to a hex string.
pub fn hash_to_hex(hash: &Hash) -> String {
    hash.to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_hash_bytes_consistency() {
        let data = b"hello world";
        let h1 = _hash_bytes(data);
        let h2 = _hash_bytes(data);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"test content").unwrap();
        let h = hash_file(f.path()).unwrap();
        assert_eq!(h.to_hex().len(), 64);
    }

    #[test]
    fn test_hash_different_files() {
        let mut f1 = tempfile::NamedTempFile::new().unwrap();
        f1.write_all(b"content a").unwrap();
        let mut f2 = tempfile::NamedTempFile::new().unwrap();
        f2.write_all(b"content b").unwrap();
        let h1 = hash_file(f1.path()).unwrap();
        let h2 = hash_file(f2.path()).unwrap();
        assert_ne!(h1, h2);
    }
}
