use std::path::PathBuf;

/// Create a fresh temporary repository with a standard set of test files.
/// Returns the TempDir (kept alive for the lifetime of the test) and the root path.
pub fn setup_repo() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("README.txt"), "ngdar test project").unwrap();
    std::fs::write(root.join("docs/note.txt"), "incremental backup test").unwrap();
    std::fs::write(root.join("large.bin"), vec![0xABu8; 1024]).unwrap();

    // Use the crate's common module to init
    crate::common::run_ngdar(&root, &["init"]).unwrap();

    (dir, root)
}
