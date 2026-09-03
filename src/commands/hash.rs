use super::*;
use crate::hash::{hash_file, hash_to_hex};

/// Hash a file and print the result.
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
