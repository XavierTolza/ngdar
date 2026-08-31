use crate::error::NgdarError;
use std::path::Path;

/// A pattern matcher for `.ngdarignore` files.
pub struct IgnoreRules {
    patterns: Vec<String>,
}

impl IgnoreRules {
    /// Load rules from a file if it exists.
    pub fn load(root: &Path) -> Result<Self, NgdarError> {
        let ignore_file = root.join(crate::config::NGDAR_IGNORE_FILE);
        let mut patterns = Vec::new();

        if ignore_file.exists() {
            let content = std::fs::read_to_string(&ignore_file)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    patterns.push(line.to_string());
                }
            }
        }

        Ok(IgnoreRules { patterns })
    }

    /// Check if a relative path should be ignored.
    /// Supports glob patterns via the `glob` crate pattern syntax.
    pub fn is_ignored(&self, rel_path: &str) -> bool {
        if rel_path.starts_with(".ngdar") || rel_path == NGDAR_IGNORE_FILE {
            return true;
        }
        self.patterns.iter().any(|pattern| {
            let pat = if !pattern.contains('/') {
                // Pattern without a slash: match anywhere in the tree
                format!("**/{}", pattern)
            } else if pattern.ends_with('/') {
                // Directory pattern: match anywhere in the tree
                format!("**/{}/**", pattern.trim_end_matches('/'))
            } else {
                pattern.clone()
            };
            glob_match(&pat, rel_path)
        })
    }
}

/// Simple glob matching. Supports `*`, `**`, and `?`.
fn glob_match(pattern: &str, path: &str) -> bool {
    // Escape literal dots
    let pattern = pattern.replace('.', "\\.");
    let pattern = pattern.replace('?', ".");
    // Replace ** with a placeholder first
    let pattern = pattern.replace("**", "☺");
    // Single * matches anything except /
    let pattern = pattern.replace('*', "[^/]*");
    // **/ (now ☺/) matches zero or more directory components
    let pattern = pattern.replace("☺/", "(.+/)?");
    // Remaining ** (standalone ☺) matches everything including /
    let pattern = pattern.replace("☺", ".*");
    let regex_str = format!("^{}$", pattern);
    if let Ok(re) = regex::Regex::new(&regex_str) {
        re.is_match(path)
    } else {
        false
    }
}

/// Collect untracked files under a root directory.
pub fn list_untracked(root: &Path, tracked: &[String]) -> Result<Vec<String>, NgdarError> {
    let ignore_rules = IgnoreRules::load(root)?;
    let mut untracked = Vec::new();

    for entry in walkdir::WalkDir::new(root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_str().unwrap_or("");
        !name.starts_with(".ngdar")
    }) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let rel_path = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| NgdarError::Other("Path prefix error".into()))?
            .to_str()
            .unwrap_or("");
        if rel_path.is_empty() {
            continue;
        }
        if ignore_rules.is_ignored(rel_path) {
            continue;
        }
        if tracked.contains(&rel_path.to_string()) {
            continue;
        }
        untracked.push(rel_path.to_string());
    }

    untracked.sort();
    Ok(untracked)
}

pub const NGDAR_IGNORE_FILE: &str = ".ngdarignore";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ignore_dot_ngdar() {
        let rules = IgnoreRules { patterns: vec![] };
        assert!(rules.is_ignored(".ngdar/HEAD"));
        assert!(rules.is_ignored(".ngdarignore"));
    }

    #[test]
    fn test_ignore_patterns() {
        let rules = IgnoreRules {
            patterns: vec!["*.log".into(), "tmp/".into()],
        };
        assert!(rules.is_ignored("debug.log"));
        assert!(rules.is_ignored("sub/debug.log"));
        assert!(rules.is_ignored("tmp/foo.txt"));
        assert!(rules.is_ignored("a/b/tmp/foo.txt"));
        assert!(!rules.is_ignored("src/main.rs"));
    }

    #[test]
    fn test_untracked() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("tracked.txt"), "").unwrap();
        std::fs::write(dir.path().join("untracked.txt"), "").unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("sub/other.txt"), "").unwrap();
        let tracked = vec!["tracked.txt".into()];
        let ut = list_untracked(dir.path(), &tracked).unwrap();
        assert!(ut.contains(&"untracked.txt".to_string()));
        assert!(ut.contains(&"sub/other.txt".to_string()));
        assert!(!ut.contains(&"tracked.txt".to_string()));
    }
}
