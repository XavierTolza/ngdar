/// Integration tests for Repository initialization and index/head operations.
use ngdar::config::Repository;
use std::path::PathBuf;

fn setup() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    (dir, root)
}

#[test]
fn test_init_and_open() {
    let (_d, root) = setup();
    let repo = Repository::init(&root).unwrap();
    assert!(root.join(".ngdar").is_dir());
    assert!(root.join(".ngdar/repository_id").exists());
    assert!(root.join(".ngdar/index").exists());
    assert!(root.join(".ngdar/HEAD").exists());
    assert!(root.join(".ngdar/objects").is_dir());
    assert!(!repo.repo_id.is_empty());

    // Re-open
    let repo2 = Repository::find(&root).unwrap();
    assert_eq!(repo.repo_id, repo2.repo_id);
}

#[test]
fn test_find_from_subdir() {
    let (_d, root) = setup();
    Repository::init(&root).unwrap();
    let sub = root.join("a").join("b");
    std::fs::create_dir_all(&sub).unwrap();
    let repo = Repository::find(&sub).unwrap();
    assert_eq!(repo.path, root);
}

#[test]
fn test_read_write_head() {
    let (_d, root) = setup();
    let repo = Repository::init(&root).unwrap();
    assert!(repo.read_head().unwrap().is_none());
    repo.write_head("abc123").unwrap();
    assert_eq!(repo.read_head().unwrap().unwrap(), "abc123");
}

#[test]
fn test_read_write_index() {
    let (_d, root) = setup();
    let repo = Repository::init(&root).unwrap();
    let entries = vec!["file1.txt".into(), "dir/file2.txt".into()];
    repo.write_index(&entries).unwrap();
    let read = repo.read_index().unwrap();
    assert_eq!(read, entries);
}
