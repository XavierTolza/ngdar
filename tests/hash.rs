/// Integration tests for `ngdar hash <file>`.
///
/// Tests the hash command via the CLI: computing hashes of files,
/// verifying output format (64-char hex), and handling of non-existent files.
mod common;

#[test]
fn test_hash_command() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("test.txt"), "ngdar hash test").unwrap();

    let out = common::run_ngdar(&root, &["hash", "test.txt"]).unwrap();
    // Output should be a 64-char hex hash followed by the filename
    assert!(
        out.len() >= 64,
        "hash output should contain at least 64 chars"
    );
    assert!(
        out.contains("test.txt"),
        "hash output should contain filename"
    );
    // BLAKE3 hash is 64 hex characters
    let first_line = out.lines().next().unwrap_or("");
    let hash_part = first_line.split_whitespace().next().unwrap_or("");
    assert_eq!(hash_part.len(), 64, "BLAKE3 hash should be 64 hex chars");
}

#[test]
fn test_hash_nonexistent_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let result = common::run_ngdar(&root, &["hash", "nonexistent.txt"]);
    assert!(result.is_err(), "hash should fail for nonexistent file");
}
