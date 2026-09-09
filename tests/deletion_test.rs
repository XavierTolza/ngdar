/// Tests for file deletion detection — staged, unstaged, packed, and extracted.
mod common;
#[path = "common/setup.rs"]
mod setup;

use std::path::Path;

/// Helper: pack all staged files into `<root>/<vol_id>.tar` and return the path.
fn pack_all(root: &Path, vol_id: &str, msg: &str) -> std::path::PathBuf {
    let tar_path = root.join(format!("{}.tar", vol_id));
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

// ---------------------------------------------------------------------------
// 1. Status detects deleted committed file as unstaged
// ---------------------------------------------------------------------------
#[test]
fn status_detects_deleted_committed_file() {
    let (_dir, root) = setup::setup_repo();

    // Add & pack all standard files
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    // Delete one file
    std::fs::remove_file(root.join("README.txt")).unwrap();

    // Status should show it as unstaged (deleted)
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("README.txt (deleted)"),
        "deleted file should appear in unstaged: {out}"
    );
    assert!(
        out.contains("(no unstaged changes)") == false,
        "there should be unstaged changes: {out}"
    );
}

// ---------------------------------------------------------------------------
// 2. `ngdar add` stages a committed file for deletion
// ---------------------------------------------------------------------------
#[test]
fn add_stages_deleted_committed_file() {
    let (_dir, root) = setup::setup_repo();

    // Add & pack once
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    // Delete a committed file, then stage the deletion
    std::fs::remove_file(root.join("README.txt")).unwrap();
    let out = common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(
        out.contains("staged for deletion: README.txt"),
        "should confirm deletion staging: {out}"
    );
    assert!(
        out.contains("Staged 1 file(s) for deletion"),
        "should show deletion count: {out}"
    );

    // Status should show it under "Staged for deletion"
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("Staged for deletion:"),
        "status should have a staged-deletion section: {out}"
    );
    assert!(
        out.contains("README.txt"),
        "staged deletion should include README.txt: {out}"
    );
    // Should NOT appear in unstaged anymore
    assert!(
        !out.contains("README.txt (deleted)"),
        "staged deletions should not appear in unstaged: {out}"
    );
}

// ---------------------------------------------------------------------------
// 3. `ngdar add <dir>` detects deletions of committed files inside the dir
// ---------------------------------------------------------------------------
#[test]
fn add_directory_detects_deletions() {
    let (_dir, root) = setup::setup_repo();
    let tmp = tempfile::TempDir::new().unwrap();

    // Add & pack all (tar written OUTSIDE the repo so it doesn't get staged)
    common::run_ngdar(&root, &["add", "."]).unwrap();
    let out = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL1",
            "--out",
            tmp.path().join("VOL1.tar").to_str().unwrap(),
            "-m",
            "first pack",
        ],
    )
    .unwrap();
    assert!(out.contains("Created archive"), "pack output: {out}");

    // Delete a file inside docs/
    std::fs::remove_file(root.join("docs/note.txt")).unwrap();

    // `add .` should stage the deletion AND still add new/changed files
    let out = common::run_ngdar(&root, &["add", "."]).unwrap();
    assert!(
        out.contains("staged for deletion: docs/note.txt"),
        "dir add should detect deletion: {out}"
    );
    assert!(
        out.contains("Staged 1 file(s) for deletion"),
        "should show deletion count: {out}"
    );

    // Status: deletion should be staged, not unstaged
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("Staged for deletion:"),
        "status should show staged deletions: {out}"
    );
    assert!(
        !out.contains("docs/note.txt (deleted)"),
        "staged deletion should not appear as unstaged: {out}"
    );
}

// ---------------------------------------------------------------------------
// 4. Pack commits the deletion and clears the staging area
// ---------------------------------------------------------------------------
#[test]
fn pack_commits_deletion() {
    let (_dir, root) = setup::setup_repo();

    // Add & pack
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    // Delete, stage, pack again
    std::fs::remove_file(root.join("README.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let out = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL2",
            "--out",
            root.join("VOL2.tar").to_str().unwrap(),
            "-m",
            "removed README.txt",
        ],
    )
    .unwrap();
    assert!(
        out.contains("Removing 1 file(s)..."),
        "pack should mention removals: {out}"
    );

    // Status should be clean (nothing staged, no unstaged changes for that file)
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "after pack, index should be empty: {out}"
    );
    assert!(
        out.contains("(no unstaged changes)"),
        "after pack deletion, no unstaged changes: {out}"
    );
}

// ---------------------------------------------------------------------------
// 5. .ngdar/committed no longer includes the deleted file after pack
// ---------------------------------------------------------------------------
#[test]
fn committed_tracking_excludes_deleted_file() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    // Confirm committed has all three files
    let committed = std::fs::read_to_string(root.join(".ngdar/committed")).unwrap();
    assert!(
        committed.contains("README.txt"),
        "README should be committed: {committed}"
    );
    assert!(
        committed.contains("docs/note.txt"),
        "note should be committed: {committed}"
    );

    std::fs::remove_file(root.join("docs/note.txt")).unwrap();
    common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    pack_all(&root, "VOL2", "removed note.txt");

    // Committed should now have README and large.bin but NOT note.txt
    let committed = std::fs::read_to_string(root.join(".ngdar/committed")).unwrap();
    assert!(
        !committed.contains("docs/note.txt"),
        "note.txt should be removed from committed: {committed}"
    );
    assert!(
        committed.contains("README.txt"),
        "README should still be committed: {committed}"
    );
}

// ---------------------------------------------------------------------------
// 6. TAR archive contains .ngdar/deleted when files are removed
// ---------------------------------------------------------------------------
#[test]
fn tar_contains_deleted_manifest() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    std::fs::remove_file(root.join("README.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let tar_path = pack_all(&root, "VOL2", "removed");

    // Extract and verify .ngdar/deleted
    let extract = common::extract_tar(&root, &tar_path, "extract_deleted");
    let deleted_path = extract.join(".ngdar/deleted");
    assert!(
        deleted_path.exists(),
        ".ngdar/deleted should exist in the TAR"
    );
    let deleted_content = std::fs::read_to_string(&deleted_path).unwrap();
    assert!(
        deleted_content.contains("README.txt"),
        "deleted manifest should list README.txt: {deleted_content}"
    );
}

// ---------------------------------------------------------------------------
// 7. Pack with only deletions (no file additions) succeeds
// ---------------------------------------------------------------------------
#[test]
fn pack_only_deletions_succeeds() {
    let (_dir, root) = setup::setup_repo();

    // Add & pack
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt"]).unwrap();
    pack_all(&root, "VOL1", "first pack");

    // Delete both files, stage, pack again — only deletions in index
    std::fs::remove_file(root.join("README.txt")).unwrap();
    std::fs::remove_file(root.join("docs/note.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt"]).unwrap();

    let out = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL2",
            "--out",
            root.join("only_deleted.tar").to_str().unwrap(),
            "-m",
            "removed both",
        ],
    )
    .unwrap();
    assert!(
        out.contains("Removing 2 file(s)..."),
        "pack should report 2 removals: {out}"
    );
    assert!(
        out.contains("Created archive"),
        "pack should succeed: {out}"
    );

    // Status clean
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(out.contains("(nothing staged)"));
}

// ---------------------------------------------------------------------------
// 8. Export includes .ngdar/deleted when the commit has deletions
// ---------------------------------------------------------------------------
#[test]
fn export_tar_contains_deleted_manifest() {
    let (_dir, root) = setup::setup_repo();

    // First commit: add & pack
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt"]).unwrap();
    let out1 = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL1",
            "--out",
            root.join("arch1.tar").to_str().unwrap(),
            "-m",
            "first",
        ],
    )
    .unwrap();
    let commit1 = out1
        .lines()
        .find(|l| l.starts_with("Commit:"))
        .unwrap()
        .strip_prefix("Commit: ")
        .unwrap()
        .trim()
        .to_string();

    // Second commit: delete & pack
    std::fs::remove_file(root.join("README.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let out2 = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL2",
            "--out",
            root.join("arch2.tar").to_str().unwrap(),
            "-m",
            "deleted README",
        ],
    )
    .unwrap();
    let commit2 = out2
        .lines()
        .find(|l| l.starts_with("Commit:"))
        .unwrap()
        .strip_prefix("Commit: ")
        .unwrap()
        .trim()
        .to_string();

    // Export commit2 and check for .ngdar/deleted
    let export_tar = root.join("export.tar");
    common::run_ngdar(
        &root,
        &["export", &commit2, "--out", export_tar.to_str().unwrap()],
    )
    .unwrap();

    let extract = common::extract_tar(&root, &export_tar, "extract_export");
    let deleted_path = extract.join(".ngdar/deleted");
    assert!(
        deleted_path.exists(),
        "export TAR should contain .ngdar/deleted"
    );
    let deleted_content = std::fs::read_to_string(&deleted_path).unwrap();
    assert!(
        deleted_content.contains("README.txt"),
        "export .ngdar/deleted should list deleted file: {deleted_content}"
    );

    // Export commit1 should NOT have .ngdar/deleted
    let export_tar1 = root.join("export1.tar");
    common::run_ngdar(
        &root,
        &["export", &commit1, "--out", export_tar1.to_str().unwrap()],
    )
    .unwrap();
    let extract1 = common::extract_tar(&root, &export_tar1, "extract_export1");
    assert!(
        !extract1.join(".ngdar/deleted").exists(),
        "first commit export should NOT have .ngdar/deleted"
    );
}

// ---------------------------------------------------------------------------
// 9. Log shows deleted files in a commit
// ---------------------------------------------------------------------------
#[test]
fn log_shows_deleted_files() {
    let (_dir, root) = setup::setup_repo();

    // First commit
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt"]).unwrap();
    let out1 = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL1",
            "--out",
            root.join("a1.tar").to_str().unwrap(),
            "-m",
            "first",
        ],
    )
    .unwrap();
    let commit1 = out1
        .lines()
        .find(|l| l.starts_with("Commit:"))
        .unwrap()
        .strip_prefix("Commit: ")
        .unwrap()
        .trim()
        .to_string();

    // Delete and pack
    std::fs::remove_file(root.join("README.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let out2 = common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "VOL2",
            "--out",
            root.join("a2.tar").to_str().unwrap(),
            "-m",
            "removed README",
        ],
    )
    .unwrap();
    let commit2 = out2
        .lines()
        .find(|l| l.starts_with("Commit:"))
        .unwrap()
        .strip_prefix("Commit: ")
        .unwrap()
        .trim()
        .to_string();

    // Log commit1 should show README.txt and docs/note.txt
    let out1 = common::run_ngdar(&root, &["log", &commit1]).unwrap();
    assert!(
        out1.contains("README.txt"),
        "commit1 should list README.txt: {out1}"
    );
    assert!(
        out1.contains("docs/note.txt"),
        "commit1 should list docs/note.txt: {out1}"
    );

    // Log commit2 should show README.txt as deleted (incremental changeset)
    let out2 = common::run_ngdar(&root, &["log", &commit2]).unwrap();
    assert!(
        out2.contains("README.txt (deleted)"),
        "commit2 log should show README.txt as deleted: {out2}"
    );
}

// ---------------------------------------------------------------------------
// 10. Full incremental extraction simulation: extract first archive,
//     extract second archive on top, apply .ngdar/deleted, verify file is gone
// ---------------------------------------------------------------------------
#[test]
fn incremental_extraction_removes_deleted_files() {
    let (_dir, root) = setup::setup_repo();

    // Commit 1: add all standard files
    common::run_ngdar(&root, &["add", "."]).unwrap();
    let tar1 = pack_all(&root, "VOL1", "first pack");

    // Commit 2: delete one file, pack again
    std::fs::remove_file(root.join("README.txt")).unwrap();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let tar2 = pack_all(&root, "VOL2", "removed README.txt");

    // Simulate incremental recovery
    let restore = root.join("restore");
    std::fs::create_dir_all(&restore).unwrap();

    // Step 1: Extract first archive — both files appear
    let output = std::process::Command::new("tar")
        .args(["-xf", tar1.to_str().unwrap()])
        .current_dir(&restore)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction 1 failed");
    assert!(
        restore.join("README.txt").exists(),
        "README.txt should exist after extracting archive1"
    );
    assert!(
        restore.join("docs/note.txt").exists(),
        "docs/note.txt should exist after extracting archive1"
    );

    // Step 2: Extract second archive on top — README.txt is NOT included
    // (archive2 only has metadata, not README.txt data)
    let output = std::process::Command::new("tar")
        .args(["-xf", tar2.to_str().unwrap()])
        .current_dir(&restore)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction 2 failed");

    // Step 3: Apply the .ngdar/deleted manifest — remove listed files
    let deleted_path = restore.join(".ngdar/deleted");
    assert!(
        deleted_path.exists(),
        ".ngdar/deleted should exist after extracting archive2"
    );
    let to_delete = std::fs::read_to_string(&deleted_path).unwrap();
    for path in to_delete.lines() {
        let full_path = restore.join(path);
        if full_path.exists() {
            std::fs::remove_file(&full_path).unwrap();
        }
    }

    // Verify: README.txt should now be gone, other files remain
    assert!(
        !restore.join("README.txt").exists(),
        "README.txt should be removed after incremental restore"
    );
    assert!(
        restore.join("docs/note.txt").exists(),
        "docs/note.txt should still exist after incremental restore"
    );
}

// ---------------------------------------------------------------------------
// 11. Unstaged deletion is no longer shown after staging the deletion
// ---------------------------------------------------------------------------
#[test]
fn unstaged_deletion_cleared_when_staged() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "."]).unwrap();
    pack_all(&root, "VOL1", "first");

    // Delete file
    std::fs::remove_file(root.join("README.txt")).unwrap();

    // Status: show unstaged deletion
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(out.contains("README.txt (deleted)"));

    // Stage it
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();

    // Status: no longer in unstaged, now in staged deletions
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        !out.contains("README.txt (deleted)"),
        "should not appear in unstaged after staging: {out}"
    );
    assert!(
        out.contains("Staged for deletion:"),
        "should appear in staged deletions: {out}"
    );
}

// ---------------------------------------------------------------------------
// 12. Deleting a file, staging it, then re-creating it cancels the deletion
// ---------------------------------------------------------------------------
#[test]
fn re_creating_file_after_staging_deletion_cancels_it() {
    let (_dir, root) = setup::setup_repo();

    // Commit file
    std::fs::write(root.join("test.txt"), "content").unwrap();
    common::run_ngdar(&root, &["add", "test.txt"]).unwrap();
    pack_all(&root, "VOL1", "first");

    // Delete, then stage deletion
    std::fs::remove_file(root.join("test.txt")).unwrap();
    common::run_ngdar(&root, &["add", "test.txt"]).unwrap();

    // Status shows staged deletion
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("Staged for deletion:"),
        "should be staged: {out}"
    );

    // Re-create the file and re-add it — should cancel deletion, stage as addition
    std::fs::write(root.join("test.txt"), "new content").unwrap();
    let out = common::run_ngdar(&root, &["add", "test.txt"]).unwrap();
    assert!(
        out.contains("added: test.txt"),
        "re-created file should be staged as addition: {out}"
    );
    assert!(
        !out.contains("staged for deletion"),
        "should NOT be staged as deletion: {out}"
    );

    // Status: test.txt should be in staged (added), not staged for deletion
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("Staged files:"),
        "should have staged files: {out}"
    );
    assert!(
        !out.contains("Staged for deletion:"),
        "should not have staged deletions: {out}"
    );
}
