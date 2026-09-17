/// Tests for `ngdar status` — staged, unstaged, untracked.
mod common;
#[path = "common/setup.rs"]
mod setup;

#[test]
fn status_after_init_shows_nothing_staged() {
    let (_dir, root) = setup::setup_repo();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "status should show no staged files: {out}"
    );
}

#[test]
fn status_shows_staged_files() {
    let (_dir, root) = setup::setup_repo();

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
fn status_shows_nothing_staged_after_commit() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::commit(&root, "DVD-001", "test");

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "after commit, nothing should be staged: {out}"
    );
}

#[test]
fn status_shows_no_unstaged_after_commit() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    common::run_ngdar(&root, &["add", "large.bin"]).unwrap();
    common::commit(&root, "DVD-001", "test");

    // After commit, add a new file — old committed files should not appear as unstaged
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

/// Files committed in an earlier commit must not reappear as untracked after
/// a later commit: a commit tree only contains the files staged for that
/// commit, so status must rely on the full committed-tracking record.
#[test]
fn status_does_not_show_earlier_committed_files_as_untracked() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::commit(&root, "vol1", "first commit");

    common::run_ngdar(&root, &["add", "docs/note.txt"]).unwrap();
    common::commit(&root, "vol1", "second commit");

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        !out.contains("? README.txt"),
        "README.txt was committed in the first commit and should not be untracked: {out}"
    );
    assert!(
        out.contains("? large.bin"),
        "large.bin was never committed and should remain untracked: {out}"
    );
    assert!(
        out.contains("(no unstaged changes)"),
        "unchanged committed files should not appear unstaged: {out}"
    );
}
