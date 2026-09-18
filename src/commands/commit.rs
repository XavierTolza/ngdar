use super::*;
use crate::objects::{build_tree_from_index, write_object, Commit, Meta};

/// Record a new commit from the staged files.
///
/// Creates a Meta object for each staged file (tagged with `vol_id`), builds
/// the Tree objects, then writes a Commit object and updates HEAD. The staging
/// index is cleared afterwards. Unlike `pack`, this does **not** produce a TAR
/// archive — packaging is a separate, later step.
pub fn commit(vol_id: &str, message: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let mut hasher = HashProxy::load(&repo.path, &repo.cache_path())?;

    // Read index
    let index = repo.read_index()?;
    if index.is_empty() {
        return Err(NgdarError::Other(
            "Nothing to commit. Use 'ngdar add' first.".to_string(),
        ));
    }

    println!("Committing {} file(s)...", index.len());
    println!("Volume ID: {}", vol_id);

    // Phase 1: Create Meta objects for each staged file
    let mut meta_hash_for_file: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut committed_entries: Vec<(String, String)> = Vec::new();

    for rel_str in &index {
        let full_path = repo.path.join(rel_str);
        let metadata = std::fs::metadata(&full_path)?;
        let size = metadata.len();
        let mtime = get_mtime(&metadata);
        let perms = format_permissions(&metadata);

        // Get file hash (from cache if possible, or compute)
        let binary_hash = hasher.hash(rel_str)?;

        committed_entries.push((binary_hash.clone(), rel_str.clone()));

        // Create Meta object
        let meta = Meta::new(
            size,
            mtime,
            perms,
            binary_hash,
            vol_id.to_string(),
            rel_str.clone(),
        );
        let meta_text = meta.to_text();
        let meta_hash = write_object(&repo.objects_path, &meta_text)?;

        meta_hash_for_file.insert(rel_str.clone(), meta_hash.clone());
    }

    // Save updated cache
    hasher.save()?;

    // Phase 2: Build Tree objects from the staged files
    let staged_refs: Vec<(&str, &str)> = meta_hash_for_file
        .iter()
        .map(|(path, hash)| (path.as_str(), hash.as_str()))
        .collect();

    let (root_tree_hash, _) = build_tree_from_index(&repo.objects_path, &staged_refs)?;

    // Phase 3: Create Commit object
    let head = repo.read_head()?;
    let author = format!(
        "{} <{}@{}>",
        std::env::var("USER").unwrap_or_else(|_| "user".into()),
        std::env::var("USER").unwrap_or_else(|_| "user".into()),
        hostname()
    );

    let commit = Commit::new(
        root_tree_hash,
        head.clone(),
        author,
        os_string(),
        format!("ngdar-{}", env!("CARGO_PKG_VERSION")),
        now(),
        message.to_string(),
    );

    let commit_text = commit.to_text();
    let commit_hash = write_object(&repo.objects_path, &commit_text)?;
    repo.write_head(&commit_hash)?;

    println!("Commit: {}", commit_hash);

    // Record committed files, then clear the staging area.
    repo.add_committed(&committed_entries)?;
    repo.clear_index()?;

    println!("Done. Index has been cleared.");
    println!("Run 'ngdar pack {}' to build the TAR archive.", commit_hash);

    Ok(())
}
