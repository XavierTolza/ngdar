/// Tests for `ngdar pack` — archive creation and TAR contents.
mod common;

use std::path::Path;
use std::process::Command;

/// Helper: add all standard files and pack into a tar, returning the tar path.
fn pack_all(root: &Path, vol_id: &str) -> std::path::PathBuf {
    common::run_ngdar(root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join(format!("session_{}.tar", vol_id));
    let out = common::run_ngdar(
        root,
        &[
            "pack",
            "--vol-id",
            vol_id,
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "test",
        ],
    )
    .unwrap();
    assert!(
        out.contains("Created archive"),
        "pack should confirm creation: {out}"
    );
    tar_path
}

/// Helper: extract a tar archive into a subdirectory and return the path.
fn extract_tar(root: &Path, tar_path: &Path, name: &str) -> std::path::PathBuf {
    let extract_dir = root.join(name);
    std::fs::create_dir_all(&extract_dir).unwrap();
    let output = Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");
    extract_dir
}

#[test]
fn pack_creates_tar_file() {
    let (_dir, root) = common::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    assert!(tar_path.exists(), "tar file should exist");
}

#[test]
fn pack_contains_metadata_and_data() {
    let (_dir, root) = common::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = extract_tar(&root, &tar_path, "extract");

    assert!(
        extract_dir.join(".ngdar").is_dir(),
        ".ngdar should be in the archive"
    );
    assert!(extract_dir.join(".ngdar/repository_id").exists());
    assert!(extract_dir.join(".ngdar/objects").is_dir());
    assert!(
        extract_dir.join("data/README.txt").exists(),
        "data/README.txt missing"
    );
    assert!(
        extract_dir.join("data/docs/note.txt").exists(),
        "data/docs/note.txt missing"
    );
    assert!(
        extract_dir.join("data/large.bin").exists(),
        "data/large.bin missing"
    );
}

#[test]
fn pack_data_files_have_correct_contents() {
    let (_dir, root) = common::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = extract_tar(&root, &tar_path, "extract");

    let readme = std::fs::read_to_string(extract_dir.join("data/README.txt")).unwrap();
    assert_eq!(readme, "ngdar test project");
    let note = std::fs::read_to_string(extract_dir.join("data/docs/note.txt")).unwrap();
    assert_eq!(note, "incremental backup test");
    let bin = std::fs::read(extract_dir.join("data/large.bin")).unwrap();
    assert_eq!(bin.len(), 1024);
    assert!(bin.iter().all(|&b| b == 0xAB));
}

#[test]
fn pack_objects_are_plain_text_meta_with_volume_id() {
    let (_dir, root) = common::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = extract_tar(&root, &tar_path, "extract");

    let objects_dir = extract_dir.join(".ngdar/objects");
    let mut has_meta = false;
    for entry in walkdir::WalkDir::new(&objects_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let content = std::fs::read_to_string(entry.path()).unwrap();
            if content.starts_with("type meta") {
                has_meta = true;
                assert!(content.contains("volume_id DVD-001"));
            }
        }
    }
    assert!(has_meta, "Should contain at least one Meta object");
}

#[test]
fn pack_error_with_empty_index() {
    let (_dir, root) = common::setup_repo();

    let tar_path = root.join("session_DVD-001.tar");
    let result = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "test",
        ],
    );
    match result {
        Err(msg) => assert!(
            msg.contains("Nothing to pack"),
            "error should mention nothing to pack: {msg}"
        ),
        Ok(_) => panic!("pack with empty index should fail"),
    }
}

#[test]
fn pack_before_init_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let result = common::run_ngdar(
        &root,
        &[
            "pack", "--vol-id", "DVD-001", "--out", "out.tar", "-m", "test",
        ],
    );
    match result {
        Err(msg) => assert!(
            msg.contains("Not an ngdar repository"),
            "error should mention repo not found: {msg}"
        ),
        Ok(_) => panic!("pack without init should fail"),
    }
}
