/// Tests for pack+status interaction — verifying that the `.ngdar/index`
/// inside a TAR archive reflects the post-commit state (empty staging area),
/// so that extracting an archive and running `ngdar status` does not show
/// committed files as "Staged".
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
        .args(["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");
    extract_dir
}

#[test]
fn pack_archive_index_is_empty_after_extraction() {
    // This test reproduces the bug where `.ngdar/index` inside the TAR archive
    // still contained staged files, causing `ngdar status` to show committed
    // files as "Staged" after extraction.
    let (_dir, root) = setup_repo();

    let tar_path = pack_all(&root, "DVD-001");
    let extract_dir = extract_tar(&root, &tar_path, "extract");

    // Verify that `.ngdar/index` inside the archive is empty
    let index_content = std::fs::read_to_string(extract_dir.join(".ngdar/index")).unwrap();
    assert!(
        index_content.trim().is_empty(),
        "Expected empty .ngdar/index in archive, got: {:?}",
        index_content
    );

    // Verify that `ngdar status` shows nothing staged
    let out = common::run_ngdar(&extract_dir, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "After extraction, status should show nothing staged: {out}"
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
