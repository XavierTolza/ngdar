/// Regression tests for commits whose files all live inside subdirectories.
///
/// A commit must produce a root Tree object that references every staged file,
/// even when no file sits directly at the repository root. Otherwise `log` and
/// `pack` see an empty tree and report zero files.
mod common;
#[path = "common/setup.rs"]
mod setup;

use std::path::Path;

/// Commit a single file located in a subdirectory and return the commit hash.
fn commit_subdir_file(root: &Path, rel: &str, vol_id: &str) -> String {
    common::run_ngdar(root, &["add", rel]).unwrap();
    common::commit(root, vol_id, "subdir only")
}

#[test]
fn log_lists_files_when_only_subdir_file_committed() {
    let (_dir, root) = setup::setup_repo();

    let commit = commit_subdir_file(&root, "docs/note.txt", "DVD-SUB");

    let out = common::run_ngdar(&root, &["log", &commit]).unwrap();
    assert!(
        out.contains("docs/note.txt"),
        "log should list docs/note.txt: {out}"
    );
    assert!(
        out.contains("Total files: 1"),
        "log should report exactly one file: {out}"
    );
}

#[test]
fn pack_includes_files_when_only_subdir_file_committed() {
    let (_dir, root) = setup::setup_repo();

    let commit = commit_subdir_file(&root, "docs/note.txt", "DVD-SUB");

    let tar = common::pack(&root, &commit, "subdir.tar");
    let ext = common::extract_tar(&root, &tar, "extract");
    common::assert_data_files(&ext, &["docs/note.txt"]);
}

#[test]
fn log_lists_files_for_deeply_nested_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();
    common::run_ngdar(&root, &["init"]).unwrap();

    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("a/b/c.txt"), "deep").unwrap();
    let commit = commit_subdir_file(&root, "a/b/c.txt", "DVD-DEEP");

    let out = common::run_ngdar(&root, &["log", &commit]).unwrap();
    assert!(
        out.contains("a/b/c.txt"),
        "log should list the deeply nested file: {out}"
    );

    let tar = common::pack(&root, &commit, "deep.tar");
    let ext = common::extract_tar(&root, &tar, "extract");
    common::assert_data_files(&ext, &["a/b/c.txt"]);
}

#[test]
fn log_lists_all_files_across_subdirs_and_root() {
    let (_dir, root) = setup::setup_repo();

    common::run_ngdar(&root, &["add", "README.txt", "docs/note.txt", "large.bin"]).unwrap();
    let commit = common::commit(&root, "DVD-MIX", "mixed");

    let out = common::run_ngdar(&root, &["log", &commit]).unwrap();
    assert!(
        out.contains("README.txt"),
        "log should list README.txt: {out}"
    );
    assert!(
        out.contains("docs/note.txt"),
        "log should list docs/note.txt: {out}"
    );
    assert!(
        out.contains("large.bin"),
        "log should list large.bin: {out}"
    );
    assert!(
        out.contains("Total files: 3"),
        "log should report three files: {out}"
    );
}
