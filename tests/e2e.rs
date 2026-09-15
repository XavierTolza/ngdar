/// Full end-to-end workflow: init -> add -> commit -> pack -> verify -> second session.
mod common;
#[path = "common/setup.rs"]
mod setup;

use common::{
    assert_data_files, assert_meta_has_volume_id, assert_no_data_files, assert_no_unstaged,
    assert_nothing_staged, assert_staged, extract_tar, run_ngdar,
};

#[test]
fn test_full_workflow() {
    let (_dir, root) = setup::setup_repo();

    // --- Session 1: add files, commit, pack, verify ---
    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(out.contains("added: README.txt"));
    assert!(out.contains("added: docs/note.txt"));
    assert!(out.contains("added: large.bin"));

    assert_staged(&root, &["README.txt", "docs/note.txt", "large.bin"]);

    let commit1 = common::commit(&root, "DVD-001", "First archival session");
    assert_nothing_staged(&root);

    let tar1 = common::pack(&root, &commit1, "session_001.tar");
    let ext1 = extract_tar(&root, &tar1, "extract");
    assert!(ext1.join(".ngdar").is_dir());
    assert_data_files(&ext1, &["README.txt", "docs/note.txt", "large.bin"]);
    assert_eq!(
        std::fs::read_to_string(ext1.join("README.txt")).unwrap(),
        "ngdar test project"
    );
    assert_meta_has_volume_id(&ext1, "DVD-001");

    // --- Session 2: add new file, commit, pack by range, verify ---
    std::fs::write(root.join("new_file.txt"), "second session file").unwrap();
    let out = common::run_ngdar(&root, &["add", "new_file.txt"]).unwrap();
    assert!(out.contains("added: new_file.txt"));

    assert_staged(&root, &["new_file.txt"]);
    assert_no_unstaged(&root);

    let commit2 = common::commit(&root, "DVD-002", "Second archival session");
    assert_nothing_staged(&root);

    // Pack only what changed between the two commits.
    let tar2 = common::pack(
        &root,
        &format!("{}..{}", commit1, commit2),
        "session_002.tar",
    );
    let ext2 = extract_tar(&root, &tar2, "extract2");

    // Data must contain only the new file (incremental).
    assert_data_files(&ext2, &["new_file.txt"]);
    assert_no_data_files(&ext2, &["README.txt", "docs/note.txt", "large.bin"]);

    // .ngdar/ must contain ALL metadata (full history), including both volume IDs.
    assert_meta_has_volume_id(&ext2, "DVD-002");
    assert_meta_has_volume_id(&ext2, "DVD-001");
}

/// Packing a single commit includes every file reachable from it.
#[test]
fn test_pack_single_commit_contains_all_files() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    std::fs::write(root.join("extra.txt"), "extra").unwrap();
    common::run_ngdar(&root, &["add", "extra.txt"]).unwrap();
    let commit = common::commit(&root, "DVD-ALL", "everything");

    let tar = common::pack(&root, &commit, "all.tar");
    let ext = extract_tar(&root, &tar, "extract");
    assert_data_files(
        &ext,
        &["README.txt", "docs/note.txt", "large.bin", "extra.txt"],
    );
}

/// Packing a range only includes changed/added files.
#[test]
fn test_pack_range_is_incremental() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    run_ngdar(&root, &["init"]).unwrap();

    std::fs::write(root.join("keep.txt"), "unchanged").unwrap();
    std::fs::write(root.join("change.txt"), "original").unwrap();
    run_ngdar(&root, &["add", "keep.txt", "change.txt"]).unwrap();
    let c1 = common::commit(&root, "VOL-1", "first");

    std::fs::write(root.join("change.txt"), "modified").unwrap();
    std::fs::write(root.join("new.txt"), "brand new").unwrap();
    run_ngdar(&root, &["add", "change.txt", "new.txt"]).unwrap();
    let c2 = common::commit(&root, "VOL-2", "second");

    let tar = common::pack(&root, &format!("{}..{}", c1, c2), "range.tar");
    let ext = extract_tar(&root, &tar, "extract");

    assert_data_files(&ext, &["change.txt", "new.txt"]);
    assert_no_data_files(&ext, &["keep.txt"]);
}

/// End-to-end test for `ngdar log` (list files in commit).
#[test]
fn test_log_shows_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("hello.txt"), "content for log test").unwrap();
    run_ngdar(&root, &["init"]).unwrap();
    run_ngdar(&root, &["add", "hello.txt"]).unwrap();

    let commit_hash = common::commit(&root, "DVD-LOG", "Test log command");

    // Now run log with commit hash
    let out = run_ngdar(&root, &["log", &commit_hash]).unwrap();
    assert!(
        out.contains("Commit:"),
        "log output should show commit: {out}"
    );
    assert!(
        out.contains("hello.txt"),
        "log should list hello.txt: {out}"
    );
    assert!(out.contains("DVD-LOG"), "log should show volume_id: {out}");
}

#[test]
fn test_log_no_commits() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    common::run_ngdar(&root, &["init"]).unwrap();
    let out = common::run_ngdar(&root, &["log"]).unwrap();
    assert!(
        out.contains("no commits"),
        "log with no commits should show message: {out}"
    );
}
