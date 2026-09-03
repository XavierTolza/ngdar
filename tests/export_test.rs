/// Integration tests for `ngdar export` — extract files from a commit.
mod common;

#[test]
fn test_export_nonexistent_commit() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    common::run_ngdar(&root, &["init"]).unwrap();

    let bad_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let result = common::run_ngdar(&root, &["export", bad_hash, "--out", "out.tar"]);
    assert!(result.is_err(), "export should fail on bad commit hash");
}
