/// Tests for `ngdar add` — staging files.
mod common;

#[test]
fn add_single_files_shown_in_output() {
    let (_dir, root) = common::setup_repo();

    let out = common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(out.contains("added: README.txt"));
    assert!(out.contains("Added 1 file(s) to staging area."));

    let out = common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    assert!(out.contains("added: docs/note.txt"));
    assert!(out.contains("Added 2 file(s) to staging area."));
}

#[test]
fn add_multiple_files_count() {
    let (_dir, root) = common::setup_repo();

    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(out.contains("added: README.txt"));
    assert!(out.contains("added: docs/note.txt"));
    assert!(out.contains("added: large.bin"));
    assert!(out.contains("Added 3 file(s) to staging area."));
}

#[test]
fn add_directory_recursively() {
    let (_dir, root) = common::setup_repo();

    let out = common::run_ngdar(&root, &["add", "docs"]).unwrap();
    assert!(out.contains("added: docs/note.txt"));
    assert!(out.contains("Added 1 file(s) to staging area."));
}

#[test]
fn add_nonexistent_file_warns_on_stderr() {
    let (_dir, root) = common::setup_repo();

    let binary = assert_cmd::cargo::cargo_bin("ngdar");
    let output = std::process::Command::new(binary)
        .args(&["add", "nonexistent.txt"])
        .current_dir(&root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "add should not fail for a missing file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Warning: 'nonexistent.txt' is not a file or directory"),
        "stderr should warn: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Added 0 file(s) to staging area."));
}

#[test]
fn add_already_staged_file_shows_message() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let out = common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(out.contains("already staged: README.txt"));
    assert!(out.contains("Added 1 file(s) to staging area."));
}

#[test]
fn add_before_init_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("f.txt"), "data").unwrap();
    let result = common::run_ngdar(&root, &["add", "f.txt"]);
    match result {
        Err(msg) => {
            assert!(
                msg.contains("Not an ngdar repository"),
                "error should mention repository not found: {msg}"
            );
        }
        Ok(_) => panic!("add without init should fail"),
    }
}
