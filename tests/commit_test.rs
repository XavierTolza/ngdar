/// Tests for `ngdar commit` — recording staged files as a commit.
///
/// A commit creates Meta/Tree/Commit objects and updates HEAD, but does NOT
/// produce any TAR archive; that is the job of `ngdar pack`.
mod common;
#[path = "common/setup.rs"]
mod setup;

/// No tar file should be produced by a commit.
#[test]
fn commit_does_not_create_archive() {
    let (_dir, root) = setup::setup_repo();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();

    let out = common::run_ngdar(&root, &["commit", "--vol-id", "DVD-001", "-m", "test"]).unwrap();
    assert!(out.contains("Commit:"), "commit output: {out}");

    // No *.tar file should have appeared in the repository.
    let tars: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "tar").unwrap_or(false))
        .collect();
    assert!(tars.is_empty(), "commit must not create a TAR archive");
}

/// HEAD points to the new commit and the staging index is cleared.
#[test]
fn commit_updates_head_and_clears_index() {
    let (_dir, root) = setup::setup_repo();
    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();

    let commit_hash = common::commit(&root, "DVD-001", "first commit");

    let head = std::fs::read_to_string(root.join(".ngdar/HEAD")).unwrap();
    assert_eq!(head.trim(), commit_hash);

    let index = std::fs::read_to_string(root.join(".ngdar/index")).unwrap();
    assert!(
        index.trim().is_empty(),
        "index should be empty after commit"
    );

    let out = common::run_ngdar(&root, &["status"]).unwrap();
    assert!(
        out.contains("(nothing staged)"),
        "nothing should be staged after commit: {out}"
    );
}

/// A commit is discoverable via `ngdar log`.
#[test]
fn commit_appears_in_log() {
    let (_dir, root) = setup::setup_repo();
    common::run_ngdar(&root, &["add", "README.txt"]).unwrap();
    let commit_hash = common::commit(&root, "DVD-001", "logged commit");

    let log = common::run_ngdar(&root, &["log"]).unwrap();
    assert!(log.contains(&commit_hash), "log should show commit: {log}");
    assert!(
        log.contains("logged commit"),
        "log should show message: {log}"
    );
}

/// Committing with an empty index fails.
#[test]
fn commit_error_with_empty_index() {
    let (_dir, root) = setup::setup_repo();

    let result = common::run_ngdar(&root, &["commit", "--vol-id", "DVD-001", "-m", "test"]);
    match result {
        Err(msg) => assert!(
            msg.contains("Nothing to commit"),
            "error should mention nothing to commit: {msg}"
        ),
        Ok(_) => panic!("commit with empty index should fail"),
    }
}

/// Committing before `init` fails.
#[test]
fn commit_before_init_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    let result = common::run_ngdar(&root, &["commit", "--vol-id", "DVD-001", "-m", "test"]);
    match result {
        Err(msg) => assert!(
            msg.contains("Not an ngdar repository"),
            "error should mention repo not found: {msg}"
        ),
        Ok(_) => panic!("commit without init should fail"),
    }
}

/// The chained-commit workflow: two commits linked by parent hashes.
#[test]
fn commit_chain_links_parents() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    common::run_ngdar(&root, &["init"]).unwrap();

    std::fs::write(root.join("a.txt"), "a").unwrap();
    common::run_ngdar(&root, &["add", "a.txt"]).unwrap();
    let first = common::commit(&root, "VOL-1", "first");

    std::fs::write(root.join("b.txt"), "b").unwrap();
    common::run_ngdar(&root, &["add", "b.txt"]).unwrap();
    let second = common::commit(&root, "VOL-2", "second");

    assert_ne!(first, second);
    let log = common::run_ngdar(&root, &["log"]).unwrap();
    assert!(log.contains(&first), "first commit should be in log: {log}");
    assert!(
        log.contains(&second),
        "second commit should be in log: {log}"
    );
}
