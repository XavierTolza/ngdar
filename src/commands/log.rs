use super::*;
use crate::objects::{self, Commit, Meta};

pub fn log(commit_hash: Option<&str>) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;

    match commit_hash {
        None => log_commits(&repo),
        Some(hash) => log_commit_files(&repo, hash),
    }
}

#[cfg(test)]
fn log_at(cwd: &Path, commit_hash: Option<&str>) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;
    match commit_hash {
        None => log_commits(&repo),
        Some(hash) => log_commit_files(&repo, hash),
    }
}

fn log_commits(repo: &Repository) -> Result<(), NgdarError> {
    let head = repo.read_head()?;
    let mut current = head;

    if current.is_none() {
        println!("(no commits yet)");
        return Ok(());
    }

    while let Some(hash) = current {
        let commit_text = objects::read_object(&repo.objects_path, &hash)?;
        let commit = Commit::from_text(&commit_text)?;

        // Format timestamp
        let ts = chrono::DateTime::from_timestamp(commit.timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| commit.timestamp.to_string());

        // First line of message
        let msg_first = commit.message.lines().next().unwrap_or(&commit.message);

        println!(
            "commit {}
Author: {}
Date:   {}

    {}
",
            hash, commit.author, ts, msg_first
        );

        current = commit.parent_hash;
    }
    Ok(())
}

fn log_commit_files(repo: &Repository, commit_hash: &str) -> Result<(), NgdarError> {
    let commit_text = objects::read_object(&repo.objects_path, commit_hash)?;
    let commit = Commit::from_text(&commit_text)?;

    // Walk the tree to find all Meta hashes -> paths
    let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
    let tree = objects::Tree::from_text(&tree_text)?;
    let mut meta_refs: Vec<(String, String)> = Vec::new(); // (meta_hash, rel_path)
    collect_meta_with_hashes(&repo.objects_path, &tree, &mut meta_refs, "")?;

    println!("Commit: {}", commit_hash);
    let msg_first = commit.message.lines().next().unwrap_or(&commit.message);
    println!("Message: {}", msg_first);
    if let Some(parent) = &commit.parent_hash {
        println!("Parent: {}", parent);
    }
    println!();
    println!("{:<8} {:<20} {:<64} Path", "Size", "Volume", "Hash");
    println!("{}", "-".repeat(120));

    for (meta_hash, _rel_path) in &meta_refs {
        let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
        if let Ok(meta) = Meta::from_text(&meta_text) {
            let size_str = if meta.size > 1024 * 1024 {
                format!("{:.1}MB", meta.size as f64 / (1024.0 * 1024.0))
            } else if meta.size > 1024 {
                format!("{:.1}KB", meta.size as f64 / 1024.0)
            } else {
                format!("{}B", meta.size)
            };
            let hash_short: String = meta_hash.chars().take(16).collect();
            println!(
                "{:<8} {:<20} {:<64} {}",
                size_str, meta.volume_id, hash_short, meta.path
            );
        }
    }

    println!("{}", "-".repeat(120));
    println!("Total files: {}", meta_refs.len());
    Ok(())
}

fn collect_meta_with_hashes(
    objects_dir: &Path,
    tree: &objects::Tree,
    results: &mut Vec<(String, String)>,
    prefix: &str,
) -> Result<(), NgdarError> {
    for entry in &tree.entries {
        let full_path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };
        if entry.kind == "meta" {
            results.push((entry.hash.clone(), full_path));
        } else if entry.kind == "tree" {
            let sub_text = objects::read_object(objects_dir, &entry.hash)?;
            let sub_tree = objects::Tree::from_text(&sub_text)?;
            collect_meta_with_hashes(objects_dir, &sub_tree, results, &full_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Repository;

    #[test]
    fn test_log_no_commits() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        Repository::init(&root).unwrap();
        let result = log_at(&root, None);
        assert!(result.is_ok(), "log() should succeed even with no commits");
    }
}
