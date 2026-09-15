//! Shared helpers for integration tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// Run `ngdar <args>` in `dir` and return stdout on success.
pub fn run_ngdar(dir: &Path, args: &[&str]) -> Result<String, String> {
    let binary = assert_cmd::cargo::cargo_bin("ngdar");
    let output = Command::new(binary)
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| format!("Failed to run ngdar: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(format!(
            "ngdar failed (exit {}): {}{}",
            output.status, stderr, stdout
        ));
    }

    Ok(stdout)
}

/// Run `ngdar commit --vol-id <vol_id> -m <msg>` and return the commit hash.
pub fn commit(dir: &Path, vol_id: &str, msg: &str) -> String {
    let out = run_ngdar(dir, &["commit", "--vol-id", vol_id, "-m", msg]).unwrap();
    out.lines()
        .find_map(|l| l.strip_prefix("Commit: "))
        .expect("commit output should contain a 'Commit: <hash>' line")
        .trim()
        .to_string()
}

/// Run `ngdar pack <target> --out <file>` and return the created tar path.
pub fn pack(dir: &Path, target: &str, out_name: &str) -> PathBuf {
    let tar_path = dir.join(out_name);
    let out = run_ngdar(dir, &["pack", target, "--out", tar_path.to_str().unwrap()]).unwrap();
    assert!(out.contains("Created archive"), "pack output: {out}");
    tar_path
}

/// Extract a tar archive into a subdirectory and return the path.
#[allow(dead_code)]
pub fn extract_tar(root: &Path, tar_path: &Path, name: &str) -> std::path::PathBuf {
    let extract_dir = root.join(name);
    std::fs::create_dir_all(&extract_dir).unwrap();
    let output = Command::new("tar")
        .args(["-xf", tar_path.to_str().unwrap()])
        .current_dir(&extract_dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "tar extraction failed");
    extract_dir
}

/// Assert that status output contains the given files in the staged list.
#[allow(dead_code)]
pub fn assert_staged(root: &Path, files: &[&str]) {
    let out = run_ngdar(root, &["status"]).unwrap();
    for f in files {
        assert!(out.contains(f), "'{f}' should appear in staged: {out}");
    }
}

/// Assert that status shows "(nothing staged)".
#[allow(dead_code)]
pub fn assert_nothing_staged(root: &Path) {
    let out = run_ngdar(root, &["status"]).unwrap();
    assert!(out.contains("(nothing staged)"), "nothing staged: {out}");
}

/// Assert that status shows "(no unstaged changes)".
#[allow(dead_code)]
pub fn assert_no_unstaged(root: &Path) {
    let out = run_ngdar(root, &["status"]).unwrap();
    assert!(out.contains("(no unstaged changes)"), "no unstaged: {out}");
}

/// Assert that an extracted archive contains data files.
#[allow(dead_code)]
pub fn assert_data_files(dir: &Path, files: &[&str]) {
    for f in files {
        assert!(dir.join(f).exists(), "{f} missing");
    }
}

/// Assert that an extracted archive does NOT contain data files.
#[allow(dead_code)]
pub fn assert_no_data_files(dir: &Path, files: &[&str]) {
    for f in files {
        assert!(!dir.join(f).exists(), "{f} should NOT be present");
    }
}

/// Assert that Meta objects with the given volume_id exist in the archive.
#[allow(dead_code)]
pub fn assert_meta_has_volume_id(extract_dir: &Path, vol_id: &str) {
    let objects_dir = extract_dir.join(".ngdar/objects");
    let mut found = false;
    for entry in walkdir::WalkDir::new(&objects_dir) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains(&format!("volume_id {}", vol_id)) {
                    found = true;
                }
            }
        }
    }
    assert!(found, "No Meta with volume_id {vol_id} found");
}
