/// Full end-to-end workflow: init -> add -> pack -> verify -> second session.
mod common;

use common::run_ngdar;
use std::path::Path;
use std::process::Command;

/// Assert that status output contains the given file in the staged list.
fn assert_staged(root: &Path, files: &[&str]) {
    let out = common::run_ngdar(root, &["status"]).unwrap();
    for f in files {
        assert!(out.contains(f), "'{f}' should appear in staged: {out}");
    }
}

/// Assert that status shows "(nothing staged)".
fn assert_nothing_staged(root: &Path) {
    let out = common::run_ngdar(root, &["status"]).unwrap();
    assert!(out.contains("(nothing staged)"), "nothing staged: {out}");
}

/// Assert that status shows "(no unstaged changes)".
fn assert_no_unstaged(root: &Path) {
    let out = common::run_ngdar(root, &["status"]).unwrap();
    assert!(out.contains("(no unstaged changes)"), "no unstaged: {out}");
}

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

/// Extract a tar archive into a subdirectory and return the path.
fn extract(root: &Path, tar_path: &Path, dir_name: &str) -> std::path::PathBuf {
    let extract_dir = root.join(dir_name);
    std::fs::create_dir_all(&extract_dir).unwrap();
    let output = Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");
    extract_dir
}

/// Assert that an extracted archive contains data files.
fn assert_data_files(extract_dir: &Path, files: &[&str]) {
    for f in files {
        assert!(extract_dir.join(f).exists(), "{f} missing");
    }
}

/// Assert that an extracted archive does NOT contain data files.
fn assert_no_data_files(extract_dir: &Path, files: &[&str]) {
    for f in files {
        assert!(!extract_dir.join(f).exists(), "{f} should NOT be present");
    }
}

/// Assert that Meta objects with the given volume_id exist in the archive.
fn assert_meta_has_volume_id(extract_dir: &Path, vol_id: &str) {
    let objects_dir = extract_dir.join(".ngdar/objects");
    let mut found = false;
    for entry in walkdir::WalkDir::new(&objects_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains(&format!("volume_id {}", vol_id)) {
                    found = true;
                }
            }
        }
    }
    assert!(found, "No Meta with volume_id {vol_id} found");
}

#[test]
fn test_full_workflow() {
    let (_dir, root) = common::setup_repo();

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

    let ext1 = extract(&root, &tar1, "extract");
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

    let ext2 = extract(&root, &tar2, "extract2");

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
