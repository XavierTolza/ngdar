use super::*;
use crate::objects::{self, Commit};

/// Remove a commit from the history chain.
///
/// The commit identified by `commit_hash` is dropped from the chain that
/// starts at HEAD and follows `parent` links. Commits newer than the removed
/// one are rewritten so that their `parent` pointer skips over it. Because
/// commit objects are content-addressed, each rewrite produces a new hash, and
/// the old commit objects (including the removed one) are deleted from the
/// object store.
///
/// `commit_hash` may be the literal string `HEAD` to target the latest commit.
pub fn remove_commit(commit_hash: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;

    let head = repo
        .read_head()?
        .ok_or_else(|| NgdarError::Other("No commits to remove".into()))?;

    let target = if commit_hash == "HEAD" {
        head.clone()
    } else {
        commit_hash.to_string()
    };

    // Walk the chain from HEAD, newest commit first.
    let mut chain: Vec<(String, Commit)> = Vec::new();
    let mut current = Some(head);
    while let Some(hash) = current {
        let text = objects::read_object(&repo.objects_path, &hash)
            .map_err(|_| NgdarError::Other(format!("Commit not found: {}", hash)))?;
        let commit = Commit::from_text(&text)?;
        current = commit.parent_hash.clone();
        chain.push((hash, commit));
    }

    let index = chain
        .iter()
        .position(|(hash, _)| *hash == target)
        .ok_or_else(|| NgdarError::Other(format!("Commit not found in history: {}", target)))?;

    let removed = chain.remove(index);

    // Commits newer than the removed one now live at chain[0..index], newest
    // first. Rebuild them from oldest to newest so each rewritten commit points
    // at the freshly-hashed version of its child.
    let mut new_parent = removed.1.parent_hash.clone();
    let mut rewritten = 0usize;
    for k in (0..index).rev() {
        let old_hash = chain[k].0.clone();
        let mut commit = chain[k].1.clone();
        commit.parent_hash = new_parent.clone();
        let new_hash = objects::write_object(&repo.objects_path, &commit.to_text())?;
        delete_object(&repo.objects_path, &old_hash)?;
        chain[k] = (new_hash.clone(), commit);
        new_parent = Some(new_hash);
        rewritten += 1;
    }

    delete_object(&repo.objects_path, &removed.0)?;

    match chain.first() {
        Some((hash, _)) => repo.write_head(hash)?,
        None => std::fs::write(&repo.head_path, "")?,
    }

    println!("Removed commit: {}", removed.0);
    if rewritten > 0 {
        println!("Rewrote {} descendant commit(s).", rewritten);
    }
    match chain.first() {
        Some((hash, _)) => println!("HEAD is now: {}", hash),
        None => println!("HEAD is now empty (no commits)."),
    }

    Ok(())
}

/// Delete an object from the store, removing its shard directory if empty.
fn delete_object(objects_dir: &Path, hash: &str) -> Result<(), NgdarError> {
    let path = objects::object_path(objects_dir, hash);
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_file(&path)?;
    if let Some(parent) = path.parent() {
        if parent.is_dir() {
            if let Ok(mut entries) = parent.read_dir() {
                if entries.next().is_none() {
                    std::fs::remove_dir(parent).ok();
                }
            }
        }
    }
    Ok(())
}
