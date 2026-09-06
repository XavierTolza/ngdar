/// Full end-to-end workflow: init -> add -> pack -> verify -> second session.
mod common;
#[path = "common/setup.rs"]
mod setup;

use common::{
    assert_data_files, assert_meta_has_volume_id, assert_no_data_files, assert_no_unstaged,
    assert_nothing_staged, assert_staged, extract_tar, run_ngdar,
};
use std::path::Path;

/// Pack staged files and return the tar path.
fn pack(root: &Path, vol_id: &str, out_name: &str, msg: &str) -> std::path::PathBuf {
    let tar_path = root.join(out_name);
    let out = common::run_ngdar(
        root,
        &[
            "pack",
            "--vol-id",
            vol_id,
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            msg,
        ],
    )
    .unwrap();
    assert!(out.contains("Created archive"), "pack output: {out}");
    tar_path
}

#[test]
fn test_full_workflow() {
    let (_dir, root) = setup::setup_repo();

    // --- Session 1: add files, pack, verify ---
    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(out.contains("added: README.txt"));
    assert!(out.contains("added: docs/note.txt"));
    assert!(out.contains("added: large.bin"));

    assert_staged(&root, &["README.txt", "docs/note.txt", "large.bin"]);

    let tar1 = pack(
        &root,
        "DVD-001",
        "session_001.tar",
        "First archival session",
    );
    assert_nothing_staged(&root);

    let ext1 = extract_tar(&root, &tar1, "extract");
    assert!(ext1.join(".ngdar").is_dir());
    assert_data_files(&ext1, &["README.txt", "docs/note.txt", "large.bin"]);
    assert_eq!(
        std::fs::read_to_string(ext1.join("README.txt")).unwrap(),
        "ngdar test project"
    );
    assert_meta_has_volume_id(&ext1, "DVD-001");

    // --- Session 2: add new file, pack again, verify ---
    std::fs::write(root.join("new_file.txt"), "second session file").unwrap();
    let out = common::run_ngdar(&root, &["add", "new_file.txt"]).unwrap();
    assert!(out.contains("added: new_file.txt"));

    assert_staged(&root, &["new_file.txt"]);
    assert_no_unstaged(&root);

    let tar2 = pack(
        &root,
        "DVD-002",
        "session_002.tar",
        "Second archival session",
    );
    assert_nothing_staged(&root);

    let ext2 = extract_tar(&root, &tar2, "extract2");

    // data/ must contain only the new file (incremental)
    assert_data_files(&ext2, &["new_file.txt"]);
    assert_no_data_files(&ext2, &["README.txt", "docs/note.txt", "large.bin"]);

    // .ngdar/ must contain ALL metadata (full history), including both volume IDs
    assert_meta_has_volume_id(&ext2, "DVD-002");
    assert_meta_has_volume_id(&ext2, "DVD-001");
}

/// End-to-end test for `ngdar log` (list files in commit).
#[test]
fn test_log_shows_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("hello.txt"), "content for log test").unwrap();
    run_ngdar(&root, &["init"]).unwrap();
    run_ngdar(&root, &["add", "hello.txt"]).unwrap();

    let tar_path = root.join("test_archive.tar");
    let out = run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-LOG",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "Test log command",
        ],
    )
    .unwrap();

    // Extract commit hash from pack output
    let commit_line = out.lines().find(|l| l.starts_with("Commit:")).unwrap();
    let commit_hash = commit_line
        .strip_prefix("Commit: ")
        .unwrap()
        .trim()
        .to_string();

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
