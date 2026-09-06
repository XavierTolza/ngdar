use super::*;
use crate::objects::{build_tree_from_index, write_object, Commit, Meta};

/// Create a TAR archive with object metadata and incremental file data.
pub fn pack(vol_id: &str, out: &str, message: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let cache_path = repo.cache_path();
    let mut cache = CacheStore::load(&cache_path)?;

    // Read index
    let index = repo.read_index()?;
    if index.is_empty() {
        return Err(NgdarError::Other(
            "Nothing to pack. Use 'ngdar add' first.".to_string(),
        ));
    }

    println!("Packing {} file(s)...", index.len());
    println!("Volume ID: {}", vol_id);

    // Ensure cache directory exists for potential new entries
    if let Some(parent) = cache_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

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
        let binary_hash = if let Some(h) = cache.lookup(size, mtime, rel_str) {
            h.to_string()
        } else {
            let hash = hash_file(&full_path)?;
            let hex = hash_to_hex(&hash);
            cache.insert(size, mtime, hex.clone(), rel_str.to_string());
            hex
        };

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
    cache.save()?;

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

    // Phase 4: Build TAR archive
    build_tar_archive(&repo, &index, out, &commit_hash, vol_id)?;

    // Record committed files for future add dedup
    repo.add_committed(&committed_entries)?;

    // Phase 5: Clear index
    repo.clear_index()?;

    println!("Created archive: {}", out);
    println!("Done. Index has been cleared.");

    Ok(())
}
