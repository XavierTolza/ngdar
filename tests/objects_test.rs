/// Integration tests for Meta, Tree, Commit objects and object I/O.
use ngdar::objects::{read_object, write_object, Commit, Meta, Tree};

#[test]
fn test_meta_roundtrip() {
    let meta = Meta::new(
        1048576,
        1788118000,
        0o644,
        "a1b2c3d4e5f6".into(),
        "DVD-001".into(),
        "docs/report.pdf".into(),
    );
    let text = meta.to_text();
    let parsed = Meta::from_text(&text).unwrap();
    assert_eq!(parsed.size, 1048576);
    assert_eq!(parsed.mtime, 1788118000);
    assert_eq!(parsed.permissions, 0o644);
    assert_eq!(parsed.binary_hash, "a1b2c3d4e5f6");
    assert_eq!(parsed.volume_id, "DVD-001");
    assert_eq!(parsed.path, "docs/report.pdf");
}

#[test]
fn test_tree_roundtrip() {
    let mut tree = Tree::new();
    tree.add("meta", "abc123".into(), "file.txt".into());
    tree.add("tree", "def456".into(), "subdir".into());
    let text = tree.to_text();
    let parsed = Tree::from_text(&text).unwrap();
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0].name, "file.txt");
    assert_eq!(parsed.entries[1].name, "subdir");
}

#[test]
fn test_commit_roundtrip() {
    let commit = Commit::new(
        "treehash123".into(),
        Some("parent456".into()),
        "Xavier".into(),
        "Linux 6.6.0-x86_64".into(),
        "ngdar-1.0.0".into(),
        1788118091,
        "Test commit message.".into(),
    );
    let text = commit.to_text();
    let parsed = Commit::from_text(&text).unwrap();
    assert_eq!(parsed.tree_hash, "treehash123");
    assert_eq!(parsed.parent_hash.unwrap(), "parent456");
    assert_eq!(parsed.message, "Test commit message.");
}

#[test]
fn test_commit_first() {
    let commit = Commit::new(
        "treehash".into(),
        None,
        "Xavier".into(),
        "Linux".into(),
        "ngdar-1.0.0".into(),
        1000,
        "Initial.".into(),
    );
    assert!(commit.parent_hash.is_none());
    let text = commit.to_text();
    assert!(text.contains("parent none"));
}

#[test]
fn test_write_read_object() {
    let dir = tempfile::TempDir::new().unwrap();
    let objects_dir = dir.path().join("objects");
    let content = "\
type meta
size 100
mtime 1000
permissions 644
binary_hash abc
volume_id DVD
path file.txt
";
    let hash = write_object(&objects_dir, content).unwrap();
    assert_eq!(hash.len(), 64);
    let read = read_object(&objects_dir, &hash).unwrap();
    assert_eq!(read, content);
}
