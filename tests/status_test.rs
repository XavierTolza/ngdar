/// Tests for `ngdar status` — staged, unstaged, untracked.
mod common;

#[test]
fn status_after_init_shows_nothing_staged() {
    let (_dir, root) = common::setup_repo();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "status should show no staged files: {out}"
    );
}

#[test]
fn status_shows_staged_files() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "large.bin"]).unwrap();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("Staged files:"),
        "status should show staged section: {out}"
    );
    assert!(
        out.contains("README.txt"),
        "status should list README.txt: {out}"
    );
    assert!(
        out.contains("docs/note.txt"),
        "status should list docs/note.txt: {out}"
    );
    assert!(
        out.contains("large.bin"),
        "status should list large.bin: {out}"
    );
    assert!(
        out.contains("(no unstaged changes)"),
        "status should show no unstaged: {out}"
    );
    assert!(
        out.contains("(no untracked files)"),
        "status should show no untracked: {out}"
    );
}

#[test]
fn status_shows_nothing_staged_after_pack() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let tar_path = root.join("s.tar");
    common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "test",
        ],
    )
    .unwrap();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "after pack, nothing should be staged: {out}"
    );
}

#[test]
fn status_shows_no_unstaged_after_pack() {
    let (_dir, root) = common::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "large.bin"]).unwrap();
    let tar_path = root.join("s.tar");
    common::run_ngdar(
        &root,
        &[
            "pack",
            "--vol-id",
            "DVD-001",
            "--out",
            tar_path.to_str().unwrap(),
            "-m",
            "test",
        ],
    )
    .unwrap();

    // After pack, add a new file — old committed files should not appear as unstaged
    std::fs::write(root.join("new_file.txt"), "second session file").unwrap();
    common::run_ngdar(&root, &["add", "new_file.txt"]).unwrap();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("new_file.txt"),
        "status should list new_file.txt in staged: {out}"
    );
    assert!(
        out.contains("(no unstaged changes)"),
        "old committed files should not appear unstaged: {out}"
    );
}
