/// Integration tests for `ngdar db-export <csv>`.
///
/// Tests the db-export command via the CLI: creating an archive and
/// then exporting the database to CSV, verifying all expected columns
/// and object types (meta, tree, commit) are present.
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
fn test_db_export() {
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
