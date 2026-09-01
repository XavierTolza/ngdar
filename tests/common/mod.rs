use std::path::Path;
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

/// Create a fresh temporary repository with a standard set of test files.
/// Returns the TempDir (kept alive for the lifetime of the test) and the root path.
#[allow(dead_code)]
pub fn setup_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().to_path_buf();

    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("README.txt"), "ngdar test project").unwrap();
    std::fs::write(root.join("docs/note.txt"), "incremental backup test").unwrap();
    std::fs::write(root.join("large.bin"), vec![0xABu8; 1024]).unwrap();

    run_ngdar(&root, &["init"]).unwrap();

    (dir, root)
}
