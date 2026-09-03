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
