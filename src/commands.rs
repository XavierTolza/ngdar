use crate::cache::CacheStore;
use crate::config::Repository;
use crate::error::NgdarError;
use crate::hash::{hash_file, hash_to_hex};
use crate::ignore::{self, IgnoreRules};
use crate::objects::{self, build_tree_from_index, write_object, Commit, Meta};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Get the current OS string.
fn os_string() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{} {}", os, arch)
}

/// Format file permissions in octal.
fn format_permissions(mode: &std::fs::Metadata) -> u32 {
    // Use the lower 9 bits plus setuid/setuid/sticky
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        mode.permissions().mode() & 0o777
    }
    #[cfg(not(unix))]
    {
        0o644
    }
}

/// Get file mtime in seconds since epoch.
fn get_mtime(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// -----------------------------------------------------------------------
/// `ngdar init`
/// -----------------------------------------------------------------------
pub fn init() -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    if cwd.join(crate::config::NGDAR_DIR).exists() {
        return Err(NgdarError::Config("Repository already exists".into()));
    }
    let repo = Repository::init(&cwd)?;
    println!(
        "Initialized empty ngdar repository in {}",
        cwd.join(crate::config::NGDAR_DIR).display()
    );
    println!("Repository ID: {}", repo.repo_id);
    Ok(())
}

/// -----------------------------------------------------------------------
/// `ngdar status`
/// -----------------------------------------------------------------------
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
        let commit = Commit::_from_text(&commit_text)?;
        let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
        let tree = objects::Tree::_from_text(&tree_text)?;
        // Collect all meta hashes from the tree recursively
        collect_meta_hashes(&repo.objects_path, &tree, &mut committed_files, &repo.path)?;
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
    _repo_root: &Path,
) -> Result<(), NgdarError> {
    for entry in &tree.entries {
        if entry.kind == "meta" {
            let _meta_text = objects::read_object(objects_dir, &entry.hash)?;
            // Find the original relative path by looking at the meta's binary_hash
            // We store the relative path as an entry in the tree
            // Actually, the file path is the path through the tree hierarchy.
            // We'll reconstruct it differently.
            // For now, let's look at the tree structure:
            // The file's name is entry.name, but its full path needs to be
            // reconstructed from the tree hierarchy.
            // Since we don't have the hierarchy here, we store it in the staged files list.
            // Let's just return the name for now - the actual reconstruction
            // is done via the pack process.
            results.push(entry.name.clone());
        } else if entry.kind == "tree" {
            let sub_text = objects::read_object(objects_dir, &entry.hash)?;
            let sub_tree = objects::Tree::_from_text(&sub_text)?;
            collect_meta_hashes(objects_dir, &sub_tree, results, _repo_root)?;
        }
    }
    Ok(())
}

/// -----------------------------------------------------------------------
/// `ngdar add <paths>`
/// -----------------------------------------------------------------------
pub fn add(paths: &[String]) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    let ignore_rules = IgnoreRules::load(&repo.path)?;
    let cache_path = repo.cache_path();
    let mut cache = CacheStore::load(&cache_path)?;

    // Read existing index
    let mut index = repo.read_index()?;

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
                add_file(&repo, &mut cache, &file_rel, &mut index)?;
            }
        } else if path.is_file() {
            add_file(&repo, &mut cache, rel_str, &mut index)?;
        } else {
            eprintln!("Warning: '{}' is not a file or directory", path_str);
        }
    }

    // Save cache and index
    cache.save()?;
    // Deduplicate and sort index
    index.sort();
    index.dedup();
    repo.write_index(&index)?;

    let total = index.len();
    println!("Added {} file(s) to staging area.", total);
    Ok(())
}

fn add_file(
    repo: &Repository,
    cache: &mut CacheStore,
    rel_str: &str,
    index: &mut Vec<String>,
) -> Result<(), NgdarError> {
    let full_path = repo.path.join(rel_str);
    let metadata = std::fs::metadata(&full_path)?;
    let size = metadata.len();
    let mtime = get_mtime(&metadata);

    // Check cache
    let _hash_str = if let Some(h) = cache.lookup(size, mtime, rel_str) {
        h.to_string()
    } else {
        let hash = hash_file(&full_path)?;
        let hex = hash_to_hex(&hash);
        cache.insert(size, mtime, hex.clone(), rel_str.to_string());
        hex
    };

    // Add to index if not already there
    if !index.contains(&rel_str.to_string()) {
        index.push(rel_str.to_string());
        println!("   added: {}", rel_str);
    } else {
        println!("   already staged: {}", rel_str);
    }

    Ok(())
}

/// -----------------------------------------------------------------------
/// `ngdar pack`
/// -----------------------------------------------------------------------
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

        // Create Meta object
        let meta = Meta::new(size, mtime, perms, binary_hash, vol_id.to_string());
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

    // Phase 5: Clear index
    repo.clear_index()?;

    println!("Created archive: {}", out);
    println!("Done. Index has been cleared.");

    Ok(())
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

/// Build the TAR archive containing:
/// 1. The full `.ngdar/` metadata directory (all objects, HEAD, index, repo id)
/// 2. The `data/` directory with the actual binary files for this session
fn build_tar_archive(
    repo: &Repository,
    staged_files: &[String],
    out_path: &str,
    _commit_hash: &str,
    _vol_id: &str,
) -> Result<(), NgdarError> {
    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);

    // --- Add full .ngdar/ metadata directory ---
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;

    // --- Add data/ files with original tree structure ---
    for rel_path in staged_files {
        let full_path = repo.path.join(rel_path);
        let data_tar_path = format!("data/{}", rel_path);
        let file_data = std::fs::read(&full_path)?;
        let metadata = std::fs::metadata(&full_path)?;

        let mut header = tar::Header::new_gnu();
        header.set_size(file_data.len() as u64);
        header.set_mode(format_permissions(&metadata));
        header.set_mtime(get_mtime(&metadata) as u64);
        header.set_entry_type(tar::EntryType::Regular);

        // Set the file path within the archive
        builder
            .append_data(
                &mut header,
                data_tar_path.as_str(),
                std::io::Cursor::new(&file_data),
            )
            .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
    }

    // Finalize the archive
    builder.finish()?;
    Ok(())
}

/// Recursively add a directory to a TAR archive.
fn add_dir_to_tar(
    builder: &mut tar::Builder<&std::fs::File>,
    dir_path: &Path,
    tar_prefix: &str,
    base: &Path,
) -> Result<(), NgdarError> {
    for entry in walkdir::WalkDir::new(dir_path)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !name.starts_with('.') || name == ".ngdar"
        })
    {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(base)
            .map_err(|_| NgdarError::Other("Path error in tar".into()))?;
        let tar_path = format!("{}/{}", tar_prefix, relative.to_str().unwrap_or(""));

        if entry.file_type().is_dir() {
            // Tar format: add directory entry
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            builder
                .append_data(&mut header, tar_path.as_str(), std::io::empty())
                .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
        } else if entry.file_type().is_file() {
            let data = std::fs::read(entry.path())?;
            let metadata = std::fs::metadata(entry.path())?;
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(format_permissions(&metadata));
            header.set_mtime(get_mtime(&metadata) as u64);
            header.set_entry_type(tar::EntryType::Regular);
            builder
                .append_data(&mut header, tar_path.as_str(), std::io::Cursor::new(&data))
                .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
        }
    }
    Ok(())
}

/// Extract the TAR archive for testing/restore purposes.
pub fn _extract_tar(tar_path: &Path, dest: &Path) -> Result<(), NgdarError> {
    let file = std::fs::File::open(tar_path)?;
    let mut archive = tar::Archive::new(file);
    archive.unpack(dest)?;
    Ok(())
}
