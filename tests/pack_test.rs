/// Tests for `ngdar pack` — archive creation and TAR contents.
mod common;

use std::process::Command;

#[test]
fn pack_creates_tar_file() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_001.tar");
    let out = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "First archival session",
        ],
    )
    .unwrap();

    assert!(out.contains("Created archive"));
    assert!(tar_path.exists(), "tar file should exist");
}

#[test]
fn pack_contains_metadata_and_data() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_001.tar");
    common::run_ngdar(
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
    )
    .unwrap();

    let extract_dir = root.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    let output = Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");

    // Verify .ngdar metadata is in the archive
    assert!(
        extract_dir.join(".ngdar").is_dir(),
        ".ngdir should be in the archive"
    );
    assert!(extract_dir.join(".ngdar/repository_id").exists());
    assert!(extract_dir.join(".ngdar/objects").is_dir());

    // Verify data files are in the archive
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

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_001.tar");
    common::run_ngdar(
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
    )
    .unwrap();

    let extract_dir = root.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();

    let readme_content = std::fs::read_to_string(extract_dir.join("data/README.txt")).unwrap();
    assert_eq!(readme_content, "ngdar test project");
    let note_content = std::fs::read_to_string(extract_dir.join("data/docs/note.txt")).unwrap();
    assert_eq!(note_content, "incremental backup test");
    let bin_content = std::fs::read(extract_dir.join("data/large.bin")).unwrap();
    assert_eq!(bin_content.len(), 1024);
    assert!(bin_content.iter().all(|&b| b == 0xAB));
}

#[test]
fn pack_objects_are_plain_text_meta_with_volume_id() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_001.tar");
    common::run_ngdar(
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
    )
    .unwrap();

    let extract_dir = root.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();

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

    let tar_path = root.join("session_001.tar");
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
    assert!(result.is_err(), "pack without add should fail");
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
    assert!(result.is_err(), "pack without init should fail");
}
