/// Tests for `ngdar remove-commit` — dropping a commit from the history chain.
mod common;
#[path = "common/setup.rs"]
mod setup;

use common::run_ngdar;
use std::path::Path;

/// Create one commit from a single file and return its hash.
fn commit_file(root: &Path, name: &str, content: &str, vol_id: &str, msg: &str) -> String {
    std::fs::write(root.join(name), content).unwrap();
    run_ngdar(root, &["add", name]).unwrap();
    common::commit(root, vol_id, msg)
}

/// Return the commit hashes shown by `ngdar log`, newest first.
fn log_hashes(root: &Path) -> Vec<String> {
    run_ngdar(root, &["log"])
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("commit ").map(|h| h.trim().to_string()))
        .collect()
}

/// Run `ngdar remove-commit` and return (success, stdout, stderr).
fn run_remove(root: &Path, hash: &str) -> (bool, String, String) {
    let binary = assert_cmd::cargo::cargo_bin("ngdar");
    let output = std::process::Command::new(binary)
        .args(["remove-commit", hash])
        .current_dir(root)
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn remove_head_drops_latest_commit() {
    let (_dir, root) = setup::setup_repo();

    let first = commit_file(&root, "README.txt", "one", "V1", "first");
    let second = commit_file(&root, "docs/note.txt", "two", "V2", "second");

    assert_eq!(log_hashes(&root), vec![second.clone(), first.clone()]);

    let out = run_ngdar(&root, &["remove-commit", "HEAD"]).unwrap();
    assert!(
        out.contains(&format!("Removed commit: {}", second)),
        "{out}"
    );
    assert!(out.contains(&format!("HEAD is now: {}", first)), "{out}");

    // The removed commit is gone; the parent is now the tip.
    assert_eq!(log_hashes(&root), vec![first]);
}

#[test]
fn remove_middle_commit_rewrites_descendants() {
    let (_dir, root) = setup::setup_repo();

    let first = commit_file(&root, "README.txt", "one", "V1", "first");
    let middle = commit_file(&root, "docs/note.txt", "two", "V2", "middle");
    let last = commit_file(&root, "large.bin", "three", "V3", "last");

    assert_eq!(
        log_hashes(&root),
        vec![last.clone(), middle.clone(), first.clone()]
    );

    let out = run_ngdar(&root, &["remove-commit", &middle]).unwrap();
    assert!(
        out.contains(&format!("Removed commit: {}", middle)),
        "{out}"
    );
    assert!(out.contains("Rewrote 1 descendant commit(s)."), "{out}");

    let after = log_hashes(&root);
    assert_eq!(after.len(), 2, "two commits should remain: {after:?}");
    assert_eq!(
        after[1], first,
        "oldest commit must be untouched: {after:?}"
    );
    assert!(
        !after.contains(&middle),
        "removed commit must be gone: {after:?}"
    );
    assert!(
        !after.contains(&last),
        "the rewritten descendant gets a new hash: {after:?}"
    );
}

#[test]
fn remove_only_commit_leaves_empty_history() {
    let (_dir, root) = setup::setup_repo();

    let only = commit_file(&root, "README.txt", "solo", "V1", "only");

    let out = run_ngdar(&root, &["remove-commit", "HEAD"]).unwrap();
    assert!(out.contains(&format!("Removed commit: {}", only)), "{out}");
    assert!(out.contains("HEAD is now empty"), "{out}");

    let log = run_ngdar(&root, &["log"]).unwrap();
    assert!(log.contains("(no commits yet)"), "log: {log}");

    let head = std::fs::read_to_string(root.join(".ngdar/HEAD")).unwrap();
    assert!(head.trim().is_empty(), "HEAD should be empty: {head:?}");
}

#[test]
fn remove_deletes_commit_object_from_store() {
    let (_dir, root) = setup::setup_repo();

    let hash = commit_file(&root, "README.txt", "gone", "V1", "to remove");
    let obj_path = root
        .join(".ngdar/objects")
        .join(&hash[..2])
        .join(&hash[2..]);
    assert!(
        obj_path.exists(),
        "commit object should exist before removal"
    );

    run_ngdar(&root, &["remove-commit", "HEAD"]).unwrap();

    assert!(
        !obj_path.exists(),
        "commit object should be deleted from the store"
    );
}

#[test]
fn remove_unknown_hash_errors() {
    let (_dir, root) = setup::setup_repo();
    commit_file(&root, "README.txt", "data", "V1", "first");

    let (ok, _stdout, stderr) = run_remove(&root, "deadbeefdeadbeef");
    assert!(!ok, "removing an unknown hash must fail");
    assert!(
        stderr.contains("Commit not found in history"),
        "stderr: {stderr}"
    );
}

#[test]
fn remove_with_no_commits_errors() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    run_ngdar(&root, &["init"]).unwrap();

    let (ok, _stdout, stderr) = run_remove(&root, "HEAD");
    assert!(!ok, "removing with no commits must fail");
    assert!(stderr.contains("No commits to remove"), "stderr: {stderr}");
}
