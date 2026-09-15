/// Tests for `ngdar add` after `ngdar commit` — verifying that unchanged
/// committed files are not re-added, while changed or new files are.
mod common;

/// Helper: commit all staged files (no TAR produced; pack is separate).
fn commit_all(root: &std::path::Path, vol_id: &str, msg: &str) {
    common::commit(root, vol_id, msg);
}

#[test]
fn add_after_commit_does_not_re_add_unchanged_files_from_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Init and create first file
    common::run_ngdar(&root, &["init"]).unwrap();
    std::fs::write(root.join("test1.txt"), "first content").unwrap();

    // First add + commit
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(out.contains("added: test1.txt"));
    assert!(out.contains("Added 1 file(s) to staging area."));

    commit_all(&root, "vol1", "first commit");

    // Create second file (test1.txt remains unchanged)
    std::fs::write(root.join("test2.txt"), "second content").unwrap();

    // Second add — test1 should NOT be re-added (unchanged + committed)
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(
        !out.contains("added: test1.txt"),
        "test1 should NOT be re-added: {out}"
    );
    assert!(
        out.contains("added: test2.txt"),
        "test2 should be added: {out}"
    );
    assert!(
        out.contains("Added 1 file(s)"),
        "only 1 file should be added: {out}"
    );
}

#[test]
fn add_after_commit_re_adds_changed_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Init and create first file
    common::run_ngdar(&root, &["init"]).unwrap();
    std::fs::write(root.join("test1.txt"), "first content").unwrap();

    // First add + commit
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(out.contains("Added 1 file(s) to staging area."));
    commit_all(&root, "vol1", "first commit");

    // Modify test1.txt (same path, different content)
    std::fs::write(root.join("test1.txt"), "modified content").unwrap();

    // Second add — test1 SHOULD be re-added because its content changed
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(
        out.contains("added: test1.txt"),
        "modified test1 should be re-added: {out}"
    );
    assert!(
        out.contains("Added 1 file(s)"),
        "only 1 file (the changed one) should be added: {out}"
    );
}

#[test]
fn add_after_commit_mixed_scenario() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Init and create two files
    common::run_ngdar(&root, &["init"]).unwrap();
    std::fs::write(root.join("stable.txt"), "stable content").unwrap();
    std::fs::write(root.join("mutable.txt"), "will change").unwrap();

    // First add + commit
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(out.contains("Added 2 file(s)"));
    commit_all(&root, "vol1", "first commit");

    // One file unchanged, one file changed, one new file added
    std::fs::write(root.join("mutable.txt"), "changed now").unwrap();
    std::fs::write(root.join("new.txt"), "brand new file").unwrap();

    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(
        !out.contains("added: stable.txt"),
        "stable.txt should NOT be re-added: {out}"
    );
    assert!(
        out.contains("added: mutable.txt"),
        "changed mutable.txt should be re-added: {out}"
    );
    assert!(
        out.contains("added: new.txt"),
        "new.txt should be added: {out}"
    );
    assert!(
        out.contains("Added 2 file(s)"),
        "2 files (changed + new) should be added: {out}"
    );
}

#[test]
fn add_after_commit_explicit_file_not_re_added_unchanged() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    common::run_ngdar(&root, &["init"]).unwrap();
    std::fs::write(root.join("data.bin"), "binary data").unwrap();

    // Add and commit a single file
    common::run_ngdar(&root, &["add", "data.bin"]).unwrap();
    commit_all(&root, "vol1", "first commit");

    // Re-add the same unchanged file
    let out = common::run_ngdar(&root, &["add", "data.bin"]).unwrap();
    assert!(
        out.contains("already committed (unchanged): data.bin"),
        "should show unchanged message: {out}"
    );
    assert!(
        out.contains("Added 0 file(s)"),
        "no files should be added: {out}"
    );
}
