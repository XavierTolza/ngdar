use super::*;

/// Stage files for the next archive.
pub fn add(paths: &[String]) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let ignore_rules = IgnoreRules::load(&repo.path)?;
    let cache_path = repo.cache_path();
    let mut cache = CacheStore::load(&cache_path)?;

    // Read existing index and committed tracking
    let mut index = repo.read_index()?;
    let committed = repo.read_committed()?;
    let mut new_count: usize = 0;

    for path_str in paths {
        let path = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            cwd.join(path_str)
        };

        // Get relative path
        let rel_path = path.strip_prefix(&repo.path).map_err(|_| {
            NgdarError::Other(format!("Path '{}' is outside the repository", path_str))
        })?;
        let rel_str = rel_path.to_str().unwrap_or("");

        if rel_str.starts_with(".ngdar") || rel_str == ".ngdarignore" {
            eprintln!("Skipping ngdar internal: {}", path_str);
            continue;
        }

        if ignore_rules.is_ignored(rel_str) {
            eprintln!("Skipping ignored: {}", path_str);
            continue;
        }

        if path.is_dir() {
            // Walk directory recursively
            for entry in walkdir::WalkDir::new(&path).into_iter().filter_entry(|e| {
                let name = e.file_name().to_str().unwrap_or("");
                !name.starts_with(".ngdar")
            }) {
                let entry = entry?;
                if entry.file_type().is_dir() {
                    continue;
                }
                let file_path = entry.path();
                let file_rel = file_path
                    .strip_prefix(&repo.path)
                    .map_err(|_| NgdarError::Other("Path error".into()))?
                    .to_str()
                    .unwrap_or("")
                    .to_string();

                if ignore_rules.is_ignored(&file_rel) {
                    continue;
                }
                if add_file(&repo, &mut cache, &committed, &file_rel, &mut index)? {
                    new_count += 1;
                }
            }
        } else if path.is_file() {
            if add_file(&repo, &mut cache, &committed, rel_str, &mut index)? {
                new_count += 1;
            }
        } else {
            return Err(NgdarError::Other(format!(
                "'{}' is not a file or directory",
                path_str
            )));
        }
    }

    // Save cache and index
    cache.save()?;
    // Deduplicate and sort index
    index.sort();
    index.dedup();
    repo.write_index(&index)?;

    println!("Added {} file(s) to staging area.", new_count);
    Ok(())
}

fn add_file(
    repo: &Repository,
    cache: &mut CacheStore,
    committed: &[(String, String)],
    rel_str: &str,
    index: &mut Vec<String>,
) -> Result<bool, NgdarError> {
    let full_path = repo.path.join(rel_str);
    let metadata = std::fs::metadata(&full_path)?;
    let size = metadata.len();
    let mtime = get_mtime(&metadata);

    // Already staged — no need to check committed
    if index.contains(&rel_str.to_string()) {
        println!("   already staged: {}", rel_str);
        return Ok(false);
    }

    // Check if this file was already committed with an unchanged hash.
    // For committed files we always compute fresh to be robust against
    // the rare case where size/mtime collide for different content.
    let hash_str = if is_committed(committed, rel_str) {
        let hash = hash_file(&full_path)?;
        let hex = hash_to_hex(&hash);
        // Seed cache if it was missing or stale
        cache.insert(size, mtime, hex.clone(), rel_str.to_string());
        hex
    } else if let Some(h) = cache.lookup(size, mtime, rel_str) {
        h.to_string()
    } else {
        let hash = hash_file(&full_path)?;
        let hex = hash_to_hex(&hash);
        cache.insert(size, mtime, hex.clone(), rel_str.to_string());
        hex
    };

    if committed
        .iter()
        .any(|(h, p)| p == rel_str && *h == hash_str)
    {
        println!("   already committed (unchanged): {}", rel_str);
        return Ok(false);
    }

    index.push(rel_str.to_string());
    println!("   added: {}", rel_str);
    Ok(true)
}

fn is_committed(committed: &[(String, String)], rel_str: &str) -> bool {
    committed.iter().any(|(_, p)| p == rel_str)
}
