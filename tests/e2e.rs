/// End-to-end tests for NGDAR CLI workflow.
///
/// Tests the full lifecycle: init → add → pack → extract → verify.
/// Also tests the utility commands: hash, archive-content, db-export, archive-remove.
use std::path::Path;
use std::process::Command;

/// Helper to run `ngdar` CLI in a given directory.
fn run_ngdar(dir: &Path, args: &[&str]) -> Result<String, String> {
    let binary = assert_cmd::cargo::cargo_bin("ngdar");
    let output = Command::new(binary)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("Failed to run ngdar: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "ngdar failed (exit {}): {}{}",
            output.status, stderr, stdout
        ));
    }

    Ok(stdout)
}

#[test]
fn test_full_workflow() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create some test files
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("README.txt"), "ngdar test project").unwrap();
    std::fs::write(root.join("docs/note.txt"), "incremental backup test").unwrap();
    std::fs::write(root.join("large.bin"), vec![0xABu8; 1024]).unwrap();

    // --- ngdar init ---
    let out = run_ngdar(&root, &["init"]).unwrap();
    assert!(out.contains("Initialized empty ngdar repository"));
    assert!(root.join(".ngdar").is_dir());
    assert!(root.join(".ngdar/repository_id").exists());

    // --- ngdar add ---
    let out = run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(out.contains("added"));
    let out = run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    assert!(out.contains("added"));
    let out = run_ngdar(&root, &["add", "large.bin"]).unwrap();
    assert!(out.contains("added"));

    // --- ngdar status (staged files) ---
    let out = run_ngdar(&root, &["status"]).unwrap();
    assert!(out.contains("Staged files"));

    // --- ngdar pack ---
    let tar_path = root.join("session_001.tar");
    let out = run_ngdar(
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
    assert!(tar_path.exists());

    // --- Verify TAR contents ---
    let extract_dir = root.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();

    // Extract using system tar
    let output = Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");

    // Verify .ngdar metadata is in the archive
    assert!(
        extract_dir.join(".ngdar").is_dir(),
        ".ngdar should be in the archive"
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

    // Verify file contents
    let readme_content = std::fs::read_to_string(extract_dir.join("data/README.txt")).unwrap();
    assert_eq!(readme_content, "ngdar test project");
    let note_content = std::fs::read_to_string(extract_dir.join("data/docs/note.txt")).unwrap();
    assert_eq!(note_content, "incremental backup test");
    let bin_content = std::fs::read(extract_dir.join("data/large.bin")).unwrap();
    assert_eq!(bin_content.len(), 1024);
    assert!(bin_content.iter().all(|&b| b == 0xAB));

    // --- Verify objects are in plain text ---
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

    // --- Second session: add more files ---
    std::fs::write(root.join("new_file.txt"), "second session file").unwrap();
    let out = run_ngdar(&root, &["add", "new_file.txt"]).unwrap();
    assert!(out.contains("added"));

    let tar2_path = root.join("session_002.tar");
    let out = run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-002",
            "--out",
            tar2_path.to_str().unwrap(),
            "-m",
            "Second archival session",
        ],
    )
    .unwrap();
    assert!(out.contains("Created archive"));
    assert!(tar2_path.exists());

    // Extract second archive
    let extract2_dir = root.join("extract2");
    std::fs::create_dir_all(&extract2_dir).unwrap();
    let output = Command::new("tar")
        .args(&["-xf", tar2_path.to_str().unwrap()])
        .current_dir(&extract2_dir)
        .output()
        .unwrap();
    assert!(output.status.success());

    // Second archive should have ALL metadata (complete history)
    assert!(extract2_dir.join(".ngdar").is_dir());
    // But only the new file in data/
    assert!(
        extract2_dir.join("data/new_file.txt").exists(),
        "data/new_file.txt missing"
    );
    // Should NOT have old files in data/
    assert!(
        !extract2_dir.join("data/README.txt").exists(),
        "data/README.txt should NOT be in incremental data"
    );

    // Verify the second archive also contains the volume ID
    let objects_dir2 = extract2_dir.join(".ngdar/objects");
    let mut found_dvd002 = false;
    for entry in walkdir::WalkDir::new(&objects_dir2) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains("volume_id DVD-002") {
                    found_dvd002 = true;
                }
            }
        }
    }
    assert!(
        found_dvd002,
        "Second archive should contain Meta with volume_id DVD-002"
    );
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
