/// Integration tests for `ngdar archive-remove --vol-id <vol_id>`.
///
/// Tests the archive-remove command via the CLI: creating two archives
/// with different volume IDs, removing one, and verifying that only the
/// matching Meta objects are deleted while the other volume's data remains.
mod common;

#[test]
fn test_archive_remove() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    // Create test files
    std::fs::write(root.join("file_a.txt"), "file a content").unwrap();
    common::run_ngdar(&root, &["init"]).unwrap();
    common::run_ngdar(&root, &["add", "file_a.txt"]).unwrap();

    let tar1 = root.join("vol1.tar");
    common::run_ngdar(
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
    common::run_ngdar(&root, &["add", "file_b.txt"]).unwrap();

    let tar2 = root.join("vol2.tar");
    common::run_ngdar(
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
    let out = common::run_ngdar(&root, &["archive-remove", "--vol-id", "DVD-001"]).unwrap();
    assert!(out.contains("Deleted"), "output should confirm deletion");
    assert!(
        out.contains("DVD-001"),
        "output should mention the removed volume"
    );

    // Verify DVD-001 Meta objects are gone by checking db-export
    let csv_path = root.join("verify.csv");
    common::run_ngdar(&root, &["db-export", csv_path.to_str().unwrap()]).unwrap();
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
