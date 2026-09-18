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

/// `ngdar status` must not print old/new hashes unless --show-hash is passed.
#[test]
fn status_hides_hashes_by_default() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::commit(&root, "DVD-001", "test");

    std::fs::write(root.join("README.txt"), "ngdar test project v2").unwrap();

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("M README.txt"),
        "README.txt should appear as modified: {out}"
    );
    assert!(
        !out.contains("→"),
        "default status should not print old → new hashes: {out}"
    );
}

/// `ngdar status --show-hash` annotates modified files with the 8-character
/// prefixes of the hash recorded in the last commit and the current on-disk
/// hash.
#[test]
fn status_show_hash_prints_old_and_new_hash_prefixes() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::commit(&root, "DVD-001", "test");

    // Hash recorded by the commit (file still unchanged on disk).
    let old_binding = common::run_ngdar(&root, &["hash", "README.txt"]).unwrap();
    let old_hash = old_binding.split(' ').next().unwrap_or("");

    std::fs::write(root.join("README.txt"), "ngdar test project v2").unwrap();
    let new_binding = common::run_ngdar(&root, &["hash", "README.txt"]).unwrap();
    let new_hash = new_binding.split(' ').next().unwrap_or("");

    let out = common::run_ngdar(&root, &["status", "--show-hash"]).unwrap();
    assert!(
        out.contains(&format!(
            "{} → {}",
            old_hash[..8].to_string(),
            new_hash[..8].to_string()
        )),
        "status --show-hash should print old → new 8-char prefixes: {out}"
    );
}

/// `ngdar status --show-hash --full-hash` prints the complete BLAKE3 hashes.
#[test]
fn status_show_hash_full_prints_full_hashes() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    common::commit(&root, "DVD-001", "test");

    let old_binding = common::run_ngdar(&root, &["hash", "README.txt"]).unwrap();
    let old_hash = old_binding.split(' ').next().unwrap_or("");

    std::fs::write(root.join("README.txt"), "ngdar test project v2").unwrap();
    let new_binding = common::run_ngdar(&root, &["hash", "README.txt"]).unwrap();
    let new_hash = new_binding.split(' ').next().unwrap_or("");

    let out = common::run_ngdar(&root, &["status", "--show-hash", "--full-hash"]).unwrap();
    assert!(
        out.contains(&format!("{} → {}", old_hash, new_hash)),
        "status --show-hash --full-hash should print the full hashes: {out}"
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
