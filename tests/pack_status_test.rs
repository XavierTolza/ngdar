/// Tests for pack+status interaction — verifying that the `.ngdar/index`
/// inside a TAR archive reflects the post-commit state (empty staging area),
/// so that extracting an archive and running `ngdar status` does not show
/// committed files as "Staged".
mod common;
#[path = "common/setup.rs"]
mod setup;

use std::path::Path;
use std::process::Command;

/// Helper: add all standard files, commit them, and pack into a tar,
/// returning the tar path.
fn pack_all(root: &Path, vol_id: &str) -> std::path::PathBuf {
    common::run_ngdar(root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let commit_hash = common::commit(root, vol_id, "test");
    common::pack(root, &commit_hash, &format!("session_{}.tar", vol_id))
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
    // The `.ngdar/index` embedded in an archive must reflect the post-commit
    // state (empty staging area), otherwise `ngdar status` on an extracted
    // archive would show committed files as "Staged".
    let (_dir, root) = setup::setup_repo();

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
