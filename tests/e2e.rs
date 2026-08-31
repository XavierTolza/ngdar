/// Full end-to-end workflow: init → add → status → pack → verify → second session.
mod common;

use std::process::Command;

#[test]
fn test_full_workflow() {
    let (_dir, root) = common::setup_repo();

    // --- ngdar add ---
    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(
        out.contains("added: README.txt"),
        "add output missing filename: {out}"
    );
    assert!(
        out.contains("added: docs/note.txt"),
        "add output missing filename: {out}"
    );
    assert!(
        out.contains("added: large.bin"),
        "add output missing filename: {out}"
    );

    // --- ngdar status ---
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(out.contains("README.txt"), "status should list README.txt");
    assert!(
        out.contains("docs/note.txt"),
        "status should list docs/note.txt"
    );
    assert!(out.contains("large.bin"), "status should list large.bin");

    // --- ngdar pack ---
    let tar_path = root.join("session_001.tar");
    let out = common::run_ngdar(
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

    // --- After pack, index should be empty ---
    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "after pack nothing staged: {out}"
    );

    // --- Verify TAR contents ---
    let extract_dir = root.join("extract");
    std::fs::create_dir_all(&extract_dir).unwrap();
    Command::new("tar")
        .args(&["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();

    assert!(extract_dir.join(".ngdar").is_dir());
    assert!(extract_dir.join("data/README.txt").exists());
    assert!(extract_dir.join("data/docs/note.txt").exists());
    assert!(extract_dir.join("data/large.bin").exists());
    let readme = std::fs::read_to_string(extract_dir.join("data/README.txt")).unwrap();
    assert_eq!(readme, "ngdar test project");

    // --- Second session: add new file, pack again ---
    std::fs::write(root.join("new_file.txt"), "second session file").unwrap();
    let out = common::run_ngdar(&root, &["add", "new_file.txt"]).unwrap();
    assert!(out.contains("added: new_file.txt"));

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("new_file.txt"),
        "status should list new_file.txt: {out}"
    );
    assert!(out.contains("(no unstaged changes)"), "no unstaged: {out}");

    let tar2_path = root.join("session_002.tar");
    common::run_ngdar(
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

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "after second pack nothing staged: {out}"
    );

    let extract2_dir = root.join("extract2");
    std::fs::create_dir_all(&extract2_dir).unwrap();
    Command::new("tar")
        .args(&["-xf", tar2_path.to_str().unwrap()])
        .current_dir(&extract2_dir)
        .output()
        .unwrap();

    // Only the new file in data/
    assert!(extract2_dir.join("data/new_file.txt").exists());
    assert!(
        !extract2_dir.join("data/README.txt").exists(),
        "old files should NOT be in incremental data"
    );

    // Second archive has full metadata with the new volume ID
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
