//! `.ngdarignore` file pattern matching and untracked file discovery.
//!
//! Provides IgnoreRules for filtering out ignored files and
//! list_untracked for finding files not yet tracked in the repository.

use crate::error::NgdarError;
use std::path::Path;

/// A pattern matcher for `.ngdarignore` files.
///
/// Supports glob patterns with `*`, `**`, and `?` wildcards.
/// Patterns without a `/` match anywhere in the directory tree;
/// patterns ending with `/` match directories.
pub struct IgnoreRules {
    /// The list of glob patterns loaded from `.ngdarignore`.
    patterns: Vec<String>,
}

impl IgnoreRules {
    /// Create a new ruleset with the given patterns.
    pub fn new(patterns: Vec<String>) -> Self {
        IgnoreRules { patterns }
    }
    /// Load rules from the `.ngdarignore` file if it exists.
    ///
    /// Blank lines and lines starting with `#` are ignored.
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

        Ok(Self::new(patterns))
    }

    /// Check if a relative path should be ignored.
    ///
    /// The `.ngdar/` directory and `.ngdarignore` file are always ignored.
    /// Supports glob patterns via the `glob_match()` function.
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
///
/// `*` matches any characters except `/`, `**` matches any characters
/// including `/`, and `?` matches a single character.
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
///
/// Walks the directory tree, skipping `.ngdar/` contents and ignored files.
/// Returns a sorted list of relative paths not present in `tracked`.
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

/// The `.ngdarignore` file name.
pub const NGDAR_IGNORE_FILE: &str = ".ngdarignore";
