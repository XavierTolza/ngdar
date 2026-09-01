/// Full end-to-end workflow: init -> add -> pack -> verify -> second session.
/// Also tests the utility commands: hash, archive-content, db-export, archive-remove.
mod common;

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
        assert!(
            extract_dir.join("data").join(f).exists(),
            "data/{f} missing"
        );
    }
}

/// Assert that an extracted archive does NOT contain data files.
fn assert_no_data_files(extract_dir: &Path, files: &[&str]) {
    for f in files {
        assert!(
            !extract_dir.join("data").join(f).exists(),
            "data/{f} should NOT be present"
        );
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
        std::fs::read_to_string(ext1.join("data/README.txt")).unwrap(),
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

/// End-to-end test for `ngdar hash <file>`.
///
/// Computes the hash of a test file and verifies the output format
/// (64-character hex string followed by the filename).
#[test]
fn test_hash_command_e2e() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("test.txt"), "ngdar hash test").unwrap();

    let out = run_ngdar(&root, &["hash", "test.txt"]).unwrap();
    // Output should be a 64-char hex hash followed by the filename
    assert!(
        out.len() >= 64,
        "hash output should contain at least 64 chars"
    );
    assert!(
        out.contains("test.txt"),
        "hash output should contain filename"
    );
    // BLAKE3 hash is 64 hex characters
    let first_line = out.lines().next().unwrap_or("");
    let hash_part = first_line.split_whitespace().next().unwrap_or("");
    assert_eq!(hash_part.len(), 64, "BLAKE3 hash should be 64 hex chars");
}

/// End-to-end test for `ngdar hash <file>` with a non-existent file.
///
/// Verifies that the command fails gracefully with an error message.
#[test]
fn test_hash_nonexistent_e2e() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let result = run_ngdar(&root, &["hash", "nonexistent.txt"]);
    assert!(result.is_err(), "hash should fail for nonexistent file");
}

/// End-to-end test for `ngdar archive-content <archive>`.
///
/// Creates a full workflow (init → add → pack), then inspects the
/// resulting archive with `archive-content` and verifies the output
/// contains expected entries.
#[test]
fn test_archive_content_e2e() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create a test file and run the full workflow
    std::fs::write(root.join("hello.txt"), "archive content test").unwrap();
    run_ngdar(&root, &["init"]).unwrap();
    run_ngdar(&root, &["add", "hello.txt"]).unwrap();

    let tar_path = root.join("test_archive.tar");
    run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-TEST-ARC",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "Test archive for content inspection",
        ],
    )
    .unwrap();

    // Now inspect the archive
    let out = run_ngdar(&root, &["archive-content", tar_path.to_str().unwrap()]).unwrap();
    assert!(
        out.contains("Archive Contents"),
        "output should show archive header"
    );
    assert!(
        out.contains("hello.txt"),
        "output should list the data file"
    );
    assert!(out.contains("data/"), "output should show data directory");
    assert!(
        out.contains(".ngdar/"),
        "output should show .ngdar metadata"
    );
}

/// End-to-end test for `ngdar db-export <csv>`.
///
/// Creates a full workflow (init → add → pack), then exports the
/// database to CSV and verifies the CSV contains all expected columns
/// and data.
#[test]
fn test_db_export_e2e() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create a test file and run the full workflow
    std::fs::write(root.join("data.bin"), "binary data for export test").unwrap();
    run_ngdar(&root, &["init"]).unwrap();
    run_ngdar(&root, &["add", "data.bin"]).unwrap();

    let tar_path = root.join("archive.tar");
    run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-EXPORT",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "Export test",
        ],
    )
    .unwrap();

    // Export the database
    let csv_path = root.join("export.csv");
    let out = run_ngdar(&root, &["db-export", csv_path.to_str().unwrap()]).unwrap();
    assert!(out.contains("Exported"), "output should confirm export");
    assert!(csv_path.exists(), "CSV file should exist");

    let csv_content = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv_content.contains("type"), "CSV should have header");
    assert!(
        csv_content.contains("meta"),
        "CSV should contain meta objects"
    );
    assert!(
        csv_content.contains("tree"),
        "CSV should contain tree objects"
    );
    assert!(
        csv_content.contains("commit"),
        "CSV should contain commit objects"
    );
    assert!(
        csv_content.contains("DVD-EXPORT"),
        "CSV should contain volume_id"
    );
}

/// End-to-end test for `ngdar archive-remove --vol-id <vol_id>`.
///
/// Creates two archives with different volume IDs, then removes one
/// and verifies that only the matching Meta objects are deleted.
#[test]
fn test_archive_remove_e2e() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create test files
    std::fs::write(root.join("file_a.txt"), "file a content").unwrap();
    run_ngdar(&root, &["init"]).unwrap();
    run_ngdar(&root, &["add", "file_a.txt"]).unwrap();

    let tar1 = root.join("vol1.tar");
    run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar1.to_str().unwrap(),
            "-m",
            "First volume",
        ],
    )
    .unwrap();

    // Second session with a different volume
    std::fs::write(root.join("file_b.txt"), "file b content").unwrap();
    run_ngdar(&root, &["add", "file_b.txt"]).unwrap();

    let tar2 = root.join("vol2.tar");
    run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-002",
            "--out",
            tar2.to_str().unwrap(),
            "-m",
            "Second volume",
        ],
    )
    .unwrap();

    // Remove DVD-001
    let out = run_ngdar(&root, &["archive-remove", "--vol-id", "DVD-001"]).unwrap();
    assert!(out.contains("Deleted"), "output should confirm deletion");
    assert!(
        out.contains("DVD-001"),
        "output should mention the removed volume"
    );

    // Verify DVD-001 Meta objects are gone by checking db-export
    let csv_path = root.join("verify.csv");
    run_ngdar(&root, &["db-export", csv_path.to_str().unwrap()]).unwrap();
    let csv_content = std::fs::read_to_string(&csv_path).unwrap();

    // The removed volume's meta should not appear
    // But the kept volume's meta should still be there
    assert!(
        !csv_content.contains("DVD-001"),
        "DVD-001 should be removed from database"
    );
    assert!(
        csv_content.contains("DVD-002"),
        "DVD-002 should still be in database"
    );
}
