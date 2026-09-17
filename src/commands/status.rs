use super::*;
use crate::hash::{hash_file, hash_to_hex};
use std::collections::HashMap;

/// A tracked file that has been modified or deleted on disk.
struct UnstagedFile {
    path: String,
    /// Most recently committed BLAKE3 hash for this path, if known.
    old_hash: Option<String>,
    /// Current on-disk BLAKE3 hash. `None` when the file was deleted or
    /// when hashes were not requested.
    new_hash: Option<String>,
    /// Whether the file was deleted from disk.
    deleted: bool,
}

/// Shorten a BLAKE3 hex hash to an 8-character prefix for display.
fn display_hash(hex: &str, full: bool) -> String {
    if full {
        hex.to_string()
    } else {
        hex.chars().take(8).collect()
    }
}

/// Show staged, unstaged, and untracked file status.
///
/// When `show_hash` is set, every modified (unstaged) file is annotated with
/// the BLAKE3 hash recorded for it in the last commit and its current on-disk
/// hash, as `<old> → <new>`. Pass `full_hash` to print the complete hashes
/// instead of an 8-character prefix.
pub fn status(show_hash: bool, full_hash: bool) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let ignore_rules = IgnoreRules::load(&repo.path)?;

    // Read current index (staged)
    let staged = repo.read_index()?;

    // Full set of tracked files: every file recorded as committed. A commit
    // tree only contains the files staged for that commit (it is not merged
    // with the parent's tree), so the HEAD tree alone is not a reliable
    // snapshot of everything that was archived over time.
    let committed = repo.read_committed()?;
    let mut committed_files: Vec<String> = committed.iter().map(|(_, path)| path.clone()).collect();
    committed_files.dedup();

    // Map each path to its most recent committed hash. The committed file is
    // append-only, so the last occurrence of a path wins.
    let mut committed_hash: HashMap<String, String> = HashMap::new();
    for (hash, path) in &committed {
        committed_hash.insert(path.clone(), hash.clone());
    }

    // Determine unstaged: files that are in committed_files but modified on disk
    let cache_path = repo.cache_path();
    let cache = CacheStore::load(&cache_path)?;
    let mut unstaged: Vec<UnstagedFile> = Vec::new();

    for rel_path in &committed_files {
        let full_path = repo.path.join(rel_path);
        if full_path.exists() {
            let metadata = std::fs::metadata(&full_path)?;
            let size = metadata.len();
            let mtime = get_mtime(&metadata);

            // Check cache
            let cached_hash = cache.lookup(size, mtime, rel_path);
            if cached_hash.is_none() {
                // File changed (different size or mtime) or not in cache
                if !staged.contains(rel_path) {
                    let new_hash = if show_hash {
                        Some(hash_to_hex(&hash_file(&full_path)?))
                    } else {
                        None
                    };
                    unstaged.push(UnstagedFile {
                        path: rel_path.clone(),
                        old_hash: committed_hash.get(rel_path).cloned(),
                        new_hash,
                        deleted: false,
                    });
                }
            }
        } else {
            // File was deleted from disk
            if !staged.contains(rel_path) {
                unstaged.push(UnstagedFile {
                    path: rel_path.clone(),
                    old_hash: committed_hash.get(rel_path).cloned(),
                    new_hash: None,
                    deleted: true,
                });
            }
        }
    }

    // Untracked files
    let untracked = ignore::list_untracked(&repo.path, &committed_files)?;
    let untracked: Vec<String> = untracked
        .into_iter()
        .filter(|p| !staged.contains(p) && !ignore_rules.is_ignored(p))
        .collect();

    // Print status
    println!("=== ngdar status ===");
    println!();

    if staged.is_empty() {
        println!("(nothing staged)");
    } else {
        println!("Staged files:");
        for f in &staged {
            println!("   \x1b[32m✓ {}\x1b[0m", f);
        }
    }
    println!();

    if unstaged.is_empty() {
        println!("(no unstaged changes)");
    } else {
        println!("Unstaged files (modified on disk):");
        for f in &unstaged {
            if f.deleted {
                println!("   \x1b[33mM {}\x1b[0m (deleted)", f.path);
            } else if show_hash {
                let old = f.old_hash.as_deref().unwrap_or("?");
                let new = f.new_hash.as_deref().unwrap_or("?");
                println!(
                    "   \x1b[33mM {}\x1b[0m  {} → {}",
                    f.path,
                    display_hash(old, full_hash),
                    display_hash(new, full_hash)
                );
            } else {
                println!("   \x1b[33mM {}\x1b[0m", f.path);
            }
        }
    }
    println!();

    if untracked.is_empty() {
        println!("(no untracked files)");
    } else {
        println!("Untracked files:");
        for f in &untracked {
            println!("   \x1b[31m? {}\x1b[0m", f);
        }
    }

    Ok(())
}
