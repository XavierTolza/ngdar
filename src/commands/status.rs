use super::*;
use crate::objects::{self, Commit};

pub fn status() -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let ignore_rules = IgnoreRules::load(&repo.path)?;

    // Read current index (staged)
    let staged = repo.read_index()?;

    // Read HEAD to get previously committed files
    let head_hash = repo.read_head()?;
    let mut committed_files: Vec<String> = Vec::new();
    if let Some(ref hash) = head_hash {
        let commit_text = objects::read_object(&repo.objects_path, hash)?;
        let commit = Commit::from_text(&commit_text)?;
        let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
        let tree = objects::Tree::from_text(&tree_text)?;
        // Collect all meta hashes from the tree recursively
        collect_meta_hashes(&repo.objects_path, &tree, &mut committed_files, "")?;
    }

    // Determine unstaged: files that are in committed_files but modified on disk
    let cache_path = repo.cache_path();
    let cache = CacheStore::load(&cache_path)?;
    let mut unstaged: Vec<String> = Vec::new();

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
                    unstaged.push(rel_path.clone());
                }
            }
        } else {
            // File was deleted from disk
            if !staged.contains(rel_path) {
                unstaged.push(format!("{} (deleted)", rel_path));
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
            println!("   \x1b[33mM {}\x1b[0m", f);
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

fn collect_meta_hashes(
    objects_dir: &Path,
    tree: &objects::Tree,
    results: &mut Vec<String>,
    prefix: &str,
) -> Result<(), NgdarError> {
    for entry in &tree.entries {
        if entry.kind == "meta" {
            let full = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", prefix, entry.name)
            };
            results.push(full);
        } else if entry.kind == "tree" {
            let sub_text = objects::read_object(objects_dir, &entry.hash)?;
            let sub_tree = objects::Tree::from_text(&sub_text)?;
            let new_prefix = if prefix.is_empty() {
                entry.name.clone()
            } else {
                format!("{}/{}", prefix, entry.name)
            };
            collect_meta_hashes(objects_dir, &sub_tree, results, &new_prefix)?;
        }
    }
    Ok(())
}
