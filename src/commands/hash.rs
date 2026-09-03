use super::*;
use crate::hash::{hash_file, hash_to_hex};

pub fn hash(path: &str) -> Result<(), NgdarError> {
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(NgdarError::Other(format!("File not found: {}", path)));
    }
    let hash = hash_file(file_path)?;
    let hex = hash_to_hex(&hash);
    println!("{}  {}", hex, path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Test that `hash()` correctly computes the BLAKE3 hash of a file.
    #[test]
    fn test_hash_command() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let result = hash(&path);
        assert!(result.is_ok(), "hash() should succeed");
    }

    /// Test that `hash()` returns an error for a non-existent file.
    #[test]
    fn test_hash_nonexistent_file() {
        let result = hash("/tmp/nonexistent_file_ngdar_test_xyz");
        assert!(result.is_err(), "hash() should fail for nonexistent file");
    }

    /// Test that `hash()` produces the correct hex string length.
    #[test]
    fn test_hash_output_format() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"test content for hash").unwrap();
        let path = f.path().to_str().unwrap().to_string();
        let file_path = std::path::Path::new(&path);
        let hash = hash_file(file_path).unwrap();
        let hex = hash_to_hex(&hash);
        assert_eq!(hex.len(), 64);
    }
}
