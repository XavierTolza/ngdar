/// Integration tests for `ngdar db-export <csv>`.
///
/// Tests the db-export command via the CLI: creating an archive and
/// then exporting the database to CSV, verifying all expected columns
/// and object types (meta, tree, commit) are present.
mod common;

#[test]
fn test_db_export() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create a test file and run the full workflow
    std::fs::write(root.join("data.bin"), "binary data for export test").unwrap();
    common::run_ngdar(&root, &["init"]).unwrap();
    common::run_ngdar(&root, &["add", "data.bin"]).unwrap();

    let tar_path = root.join("archive.tar");
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
