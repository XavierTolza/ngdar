/// Tests for `ngdar add` — staging files.
mod common;

#[test]
fn add_shows_filenames_in_output() {
    let (_dir, root) = common::setup_repo();

    let out = common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(
        out.contains("added: README.txt"),
        "add output missing filename: {out}"
    );

    let out = common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    assert!(
        out.contains("added: docs/note.txt"),
        "add output missing filename: {out}"
    );

    let out = common::run_ngdar(&root, &["add", "large.bin"]).unwrap();
    assert!(
        out.contains("added: large.bin"),
        "add output missing filename: {out}"
    );
}

#[test]
fn add_shows_total_count() {
    let (_dir, root) = common::setup_repo();

    let out =
        common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    assert!(
        out.contains("Added 3 file(s) to staging area."),
        "add output missing count: {out}"
    );
}

#[test]
fn add_already_staged_file_shows_message() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let out = common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    assert!(
        out.contains("already staged: README.txt"),
        "second add should say 'already staged': {out}"
    );
}

#[test]
fn add_before_init_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::write(root.join("f.txt"), "data").unwrap();
    let result = common::run_ngdar(&root, &["add", "f.txt"]);
    assert!(result.is_err(), "add without init should fail");
}
