/// Tests for `ngdar pack` — archive creation and TAR contents.
mod common;
#[path = "common/setup.rs"]
mod setup;

use std::path::Path;

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

#[test]
fn pack_creates_tar_file() {
    let (_dir, root) = setup::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    assert!(tar_path.exists(), "tar file should exist");
}

#[test]
fn pack_contains_metadata_and_data() {
    let (_dir, root) = setup::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = common::extract_tar(&root, &tar_path, "extract");

    assert!(
        extract_dir.join(".ngdar").is_dir(),
        ".ngdar should be in the archive"
    );
    assert!(extract_dir.join(".ngdar/repository_id").exists());
    assert!(extract_dir.join(".ngdar/objects").is_dir());
    assert!(
        extract_dir.join("README.txt").exists(),
        "README.txt missing"
    );
    assert!(
        extract_dir.join("docs/note.txt").exists(),
        "docs/note.txt missing"
    );
    assert!(extract_dir.join("large.bin").exists(), "large.bin missing");
}

#[test]
fn pack_data_files_have_correct_contents() {
    let (_dir, root) = setup::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = common::extract_tar(&root, &tar_path, "extract");

    let readme = std::fs::read_to_string(extract_dir.join("README.txt")).unwrap();
    assert_eq!(readme, "ngdar test project");
    let note = std::fs::read_to_string(extract_dir.join("docs/note.txt")).unwrap();
    assert_eq!(note, "incremental backup test");
    let bin = std::fs::read(extract_dir.join("large.bin")).unwrap();
    assert_eq!(bin.len(), 1024);
    assert!(bin.iter().all(|&b| b == 0xAB));
}

#[test]
fn pack_objects_are_plain_text_meta_with_volume_id() {
    let (_dir, root) = setup::setup_repo();
    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = common::extract_tar(&root, &tar_path, "extract");

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
    let (_dir, root) = setup::setup_repo();

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

#[test]
fn pack_reports_total_size() {
    let (_dir, root) = setup_repo();
    let tar_path = root.join("session_DVD-001.tar");
    // 18 (README.txt) + 23 (docs/note.txt) + 1024 (large.bin) = 1065 bytes
    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(out.contains("Added 3 file(s)"), "add output: {out}");

    let out = common::run_ngdar(
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
    assert!(
        out.contains("Total size: 1.0 KB"),
        "pack should report total size: {out}"
    );
}

#[test]
fn pack_verbose_lists_added_files() {
    let (_dir, root) = setup_repo();
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_DVD-001.tar");

    let out = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "test",
            "--verbose",
        ],
    )
    .unwrap();

    assert!(
        out.contains("adding: README.txt"),
        "verbose should list README.txt: {out}"
    );
    assert!(
        out.contains("adding: docs/note.txt"),
        "verbose should list docs/note.txt: {out}"
    );
    assert!(
        out.contains("adding: large.bin"),
        "verbose should list large.bin: {out}"
    );
}

#[test]
fn pack_without_verbose_does_not_list_files() {
    let (_dir, root) = setup_repo();
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let tar_path = root.join("session_DVD-001.tar");

    let out = common::run_ngdar(
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

    assert!(
        !out.contains("adding:"),
        "non-verbose pack should not list files: {out}"
    );
}

fn setup_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("README.txt"), "ngdar test project").unwrap();
    std::fs::write(root.join("docs/note.txt"), "incremental backup test").unwrap();
    std::fs::write(root.join("large.bin"), vec![0xABu8; 1024]).unwrap();

    common::run_ngdar(&root, &["init"]).unwrap();

    (dir, root)
}
