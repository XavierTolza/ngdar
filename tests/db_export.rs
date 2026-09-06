/// Integration tests for `ngdar db-export <csv>`.
///
/// Tests the db-export command via the CLI: creating an archive and
/// then exporting the database to CSV, verifying all expected columns
/// and file-entry records are present.
mod common;

#[test]
fn test_db_export() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    let tmp = tempfile::TempDir::new().unwrap();

    // Create a test file and run the full workflow
    std::fs::write(root.join("data.bin"), "binary data for export test").unwrap();
    common::run_ngdar(&root, &["init"]).unwrap();
    common::run_ngdar(&root, &["add", "data.bin"]).unwrap();

    let tar_path = tmp.path().join("archive.tar");
    common::run_ngdar(
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
    let out = common::run_ngdar(&root, &["db-export", csv_path.to_str().unwrap()]).unwrap();
    assert!(
        out.contains("Exported"),
        "output should confirm export: {out}"
    );
    assert!(csv_path.exists(), "CSV file should exist");

    let csv_content = std::fs::read_to_string(&csv_path).unwrap();
    // Check header
    assert!(
        csv_content.contains("commit_date"),
        "CSV should have commit_date header"
    );
    assert!(
        csv_content.contains("commit_hash"),
        "CSV should have commit_hash header"
    );
    assert!(
        csv_content.contains("filepath"),
        "CSV should have filepath header"
    );
    assert!(
        csv_content.contains("file_hash"),
        "CSV should have file_hash header"
    );
    assert!(
        csv_content.contains("file_size"),
        "CSV should have file_size header"
    );
    assert!(
        csv_content.contains("volume_id"),
        "CSV should have volume_id header"
    );
    // Check data
    assert!(
        csv_content.contains("data.bin"),
        "CSV should contain file path"
    );
    assert!(
        csv_content.contains("Export test"),
        "CSV should contain commit message"
    );
    assert!(
        csv_content.contains("DVD-EXPORT"),
        "CSV should contain volume_id value"
    );
}
