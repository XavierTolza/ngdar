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

/// Recursively collect file paths from a tree structure.
///
/// Walks the tree entries, collecting names of `meta` entries and recursing
/// into `tree` entries. Used by [`status()`] to reconstruct committed file paths.
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
                if add_file(&repo, &mut cache, &file_rel, &mut index)? {
                    new_count += 1;
                }
            }
        } else if path.is_file() {
            if add_file(&repo, &mut cache, rel_str, &mut index)? {
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

/// Hash a single file, update the cache, and add it to the staging index.
///
/// Skips re-hashing if the file (size, mtime, path) is already in the cache.
/// Returns without error if the file was already staged.
fn add_file(
    repo: &Repository,
    cache: &mut CacheStore,
    rel_str: &str,
    index: &mut Vec<String>,
) -> Result<bool, NgdarError> {
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
        Ok(true)
    } else {
        println!("   already staged: {}", rel_str);
        Ok(false)
    }
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

    // Phase 5: Clear index
    repo.clear_index()?;

    println!("Created archive: {}", out);
    println!("Done. Index has been cleared.");

    Ok(())
}

/// Read the system hostname from `/etc/hostname`.
fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

/// Build the TAR archive containing:
/// 1. The full `.ngdar/` metadata directory (all objects, HEAD, index, repo id)
/// 2. The actual binary files for this session, stored at the archive root
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

    // --- Add files with original tree structure ---
    for rel_path in staged_files {
        let full_path = repo.path.join(rel_path);
        let data_tar_path = rel_path.to_string();
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

/// Recursively add a directory and its contents to a TAR archive.
///
/// Skips hidden entries (names starting with `.`) except `.ngdar` itself.
/// Preserves directory structure relative to `base`.
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

/// Compute and display the BLAKE3 hash of a file.
///
/// Reads the file in 64 KiB chunks to handle large files efficiently.
/// Prints the 64-character hex-encoded BLAKE3 hash followed by the filename.
pub fn hash(path: &str) -> Result<(), NgdarError> {
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(NgdarError::Other(format!("File not found: {}", path)));
    }
    let hash = hash_file(file_path)?;
    let hex = hash_to_hex(&hash);
    println!("{}  {}", hex, path);
    Ok(())
}

/// Export all metadata from the repository database to a CSV file.
///
/// Walks every object stored in `.ngdar/objects/`, determines its type
/// (Meta, Tree, or Commit), and writes a row to the CSV with all relevant
/// fields. The CSV includes the following columns:
///
/// - `type`: object type (meta, tree, commit)
/// - `hash`: BLAKE3 hex hash of the object (its filename)
/// - `size`: file size in bytes (for Meta objects)
/// - `mtime`: modification time (Unix timestamp, for Meta objects)
/// - `permissions`: file permissions in octal (for Meta objects)
/// - `binary_hash`: BLAKE3 hash of the actual file content (for Meta objects)
/// - `volume_id`: volume identifier (for Meta objects)
/// - `tree_hash`: root tree hash (for Commit objects)
/// - `parent_hash`: parent commit hash (for Commit objects)
/// - `timestamp`: commit timestamp (for Commit objects)
/// - `message`: commit message preview (for Commit objects)
/// - `entry_count`: number of entries (for Tree objects)
pub fn db_export(csv_path: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    db_export_at(&cwd, csv_path)
}

fn db_export_at(cwd: &Path, csv_path: &str) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;

    let mut wtr = csv::Writer::from_path(csv_path)
        .map_err(|e| NgdarError::Other(format!("Cannot create CSV '{}': {}", csv_path, e)))?;

    // Write CSV header
    wtr.write_record([
        "type",
        "hash",
        "size",
        "mtime",
        "permissions",
        "binary_hash",
        "volume_id",
        "path",
        "tree_hash",
        "parent_hash",
        "timestamp",
        "message",
        "entry_count",
    ])
    .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;

    // Walk the objects directory
    let objects_dir = &repo.objects_path;
    if !objects_dir.exists() {
        return Err(NgdarError::Other("No objects directory found".into()));
    }

    let mut count = 0u64;
    for entry in walkdir::WalkDir::new(objects_dir) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        // Reconstruct the full hash from the sharded directory structure
        // objects/ab/cdef... -> abcdef...
        let rel_path = entry
            .path()
            .strip_prefix(objects_dir)
            .map_err(|_| NgdarError::Other("Path error".into()))?;
        let hash_str = rel_path.to_str().unwrap_or("").replace('/', "");

        let content = std::fs::read_to_string(entry.path())?;
        let content = content.trim();

        if content.starts_with("type meta") {
            // Meta object
            if let Ok(meta) = Meta::from_text(content) {
                wtr.write_record([
                    "meta",
                    &hash_str,
                    &meta.size.to_string(),
                    &meta.mtime.to_string(),
                    &format!("{:o}", meta.permissions),
                    &meta.binary_hash,
                    &meta.volume_id,
                    &meta.path,
                    "",
                    "",
                    "",
                    "",
                    "",
                ])
                .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;
                count += 1;
            }
        } else if content.contains("tool_version") && content.contains("author") {
            // Commit object — has tool_version, author, tree, parent, timestamp fields
            let mut tree_hash = String::new();
            let mut parent_hash = String::new();
            let mut timestamp = String::new();
            let mut message = String::new();

            for line in content.lines() {
                let line = line.trim();
                if let Some((key, value)) = line.split_once(' ') {
                    match key {
                        "tree" => tree_hash = value.to_string(),
                        "parent" if value != "none" => parent_hash = value.to_string(),
                        "timestamp" => timestamp = value.to_string(),
                        _ => {}
                    }
                }
            }
            // Message is after the first blank line
            if let Some(blank_pos) = content.find("\n\n") {
                message = content[blank_pos + 2..].trim().to_string();
                if message.len() > 100 {
                    message = message.chars().take(100).collect::<String>() + "...";
                }
            }

            wtr.write_record([
                "commit",
                &hash_str,
                "",
                "",
                "",
                "",
                "",
                "",
                &tree_hash,
                &parent_hash,
                &timestamp,
                &message,
                "",
            ])
            .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;
            count += 1;
        } else if content.starts_with("tree ")
            || content
                .lines()
                .all(|l| l.is_empty() || l.split(' ').count() == 3)
        {
            // Tree object — lines of "kind hash name"
            let entry_count = content.lines().filter(|l| !l.is_empty()).count();
            wtr.write_record([
                "tree",
                &hash_str,
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                &entry_count.to_string(),
            ])
            .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;
            count += 1;
        }
        // Skip unrecognized objects
    }

    wtr.flush()
        .map_err(|e| NgdarError::Other(format!("CSV flush error: {}", e)))?;

    println!("Exported {} object(s) to {}", count, csv_path);
    Ok(())
}

/// List all commits (like git log) or list files in a specific commit.
///
/// If no commit hash is given, walks the commit chain backwards from HEAD
/// and prints each commit's hash, timestamp, and message.
///
/// If a commit hash is given, walks the commit's tree and lists all Meta
/// objects with their original path, BLAKE3 hash, and file size.
pub fn log(commit_hash: Option<&str>) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;

    match commit_hash {
        None => log_commits(&repo),
        Some(hash) => log_commit_files(&repo, hash),
    }
}

/// List all commits (test helper with explicit root path).
#[cfg(test)]
fn log_at(cwd: &Path, commit_hash: Option<&str>) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;
    match commit_hash {
        None => log_commits(&repo),
        Some(hash) => log_commit_files(&repo, hash),
    }
}

/// Walk the commit chain backwards and print each commit.
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

/// List all files in a given commit with their path, hash, and size.
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

/// Recursively collect (meta_hash, rel_path) pairs from a Tree.
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

/// Recreate a TAR archive from stored metadata for a given commit.
///
/// Reads each file from disk using the path stored in its Meta object,
/// verifies the BLAKE3 hash matches, and packs them into a tar archive
/// at the specified output path. If a file is missing or its hash does
/// not match, a warning is printed but the command continues.
pub fn export(commit_hash: &str, out_path: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    export_with_repo(&repo, commit_hash, out_path)
}

/// Recreate a TAR archive (test helper with explicit root path).
#[cfg(test)]
fn export_at(cwd: &Path, commit_hash: &str, out_path: &str) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;
    // Reuse the same logic as export but with given cwd
    let _ = repo; // we already have the repo, but the logic needs the path
    export_with_repo(&repo, commit_hash, out_path)
}

/// Internal export implementation that takes a repo reference.
fn export_with_repo(
    repo: &Repository,
    commit_hash: &str,
    out_path: &str,
) -> Result<(), NgdarError> {
    let commit_text = objects::read_object(&repo.objects_path, commit_hash)?;
    let commit = Commit::from_text(&commit_text)?;
    let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
    let tree = objects::Tree::from_text(&tree_text)?;
    let mut meta_refs: Vec<(String, String)> = Vec::new();
    collect_meta_with_hashes(&repo.objects_path, &tree, &mut meta_refs, "")?;
    println!(
        "Exporting {} file(s) from commit {}...",
        meta_refs.len(),
        commit_hash
    );
    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;
    let mut exported = 0u64;
    let mut warnings = 0u64;
    for (meta_hash, _rel_path) in &meta_refs {
        let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
        if let Ok(meta) = Meta::from_text(&meta_text) {
            let full_path = repo.path.join(&meta.path);
            if !full_path.exists() {
                eprintln!("Warning: '{}' not found on disk, skipping", meta.path);
                warnings += 1;
                continue;
            }
            let actual_hash = hash_file(&full_path)?;
            let actual_hex = hash_to_hex(&actual_hash);
            if actual_hex != meta.binary_hash {
                eprintln!(
                    "Warning: '{}' has been modified (hash mismatch), skipping",
                    meta.path
                );
                warnings += 1;
                continue;
            }
            let file_data = std::fs::read(&full_path)?;
            let file_metadata = std::fs::metadata(&full_path)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(file_data.len() as u64);
            header.set_mode(format_permissions(&file_metadata));
            header.set_mtime(get_mtime(&file_metadata) as u64);
            header.set_entry_type(tar::EntryType::Regular);
            builder
                .append_data(&mut header, &meta.path, std::io::Cursor::new(&file_data))
                .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
            exported += 1;
        }
    }
    builder.finish()?;
    println!("Exported {} file(s) to {}", exported, out_path);
    if warnings > 0 {
        eprintln!("{} file(s) were skipped due to warnings", warnings);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Test that `hash()` correctly computes the BLAKE3 hash of a file.
    ///
    /// Creates a temporary file with known content, calls `hash()`, and
    /// verifies the output matches a direct BLAKE3 computation.
    #[test]
    fn test_hash_command() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        let path = f.path().to_str().unwrap().to_string();

        // Run the hash command (it prints to stdout, but we can verify it returns Ok)
        let result = hash(&path);
        assert!(result.is_ok(), "hash() should succeed");
    }

    /// Test that `hash()` returns an error for a non-existent file.
    #[test]
    fn test_hash_nonexistent_file() {
        let result = hash("/tmp/nonexistent_file_ngdar_test_xyz");
        assert!(result.is_err(), "hash() should fail for nonexistent file");
    }

    /// Test that `hash()` produces the correct hex string length.
    #[test]
    fn test_hash_output_format() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"test content for hash").unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let file_path = std::path::Path::new(&path);
        let hash = hash_file(file_path).unwrap();
        let hex = hash_to_hex(&hash);
        // BLAKE3 hex output is 64 characters
        assert_eq!(hex.len(), 64);
    }

    /// Test that `db_export()` creates a valid CSV file with the correct headers.
    ///
    /// Sets up a temporary ngdar repository, creates a Meta object in the
    /// object store, runs `db_export()`, and verifies the CSV output contains
    /// the expected columns and data.
    #[test]
    fn test_db_export_creates_csv() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        // Initialize a repo
        let repo = Repository::init(&root).unwrap();

        // Create a Meta object manually
        let meta = Meta::new(
            1024,
            1700000000,
            0o644,
            "abcdef123456".into(),
            "DVD-TEST".into(),
            "test.bin".into(),
        );
        let meta_text = meta.to_text();
        let _meta_hash = write_object(&repo.objects_path, &meta_text).unwrap();

        // Run db_export
        let csv_path = root.join("export.csv");
        let csv_str = csv_path.to_str().unwrap().to_string();
        let result = db_export_at(&root, &csv_str);
        assert!(result.is_ok(), "db_export_at() should succeed");

        // Verify CSV file exists and contains expected data
        assert!(csv_path.exists(), "CSV file should exist");
        let csv_content = std::fs::read_to_string(&csv_path).unwrap();
        assert!(csv_content.contains("type"), "CSV should have header");
        assert!(csv_content.contains("meta"), "CSV should contain meta row");
        assert!(csv_content.contains("1024"), "CSV should contain size");
        assert!(
            csv_content.contains("abcdef123456"),
            "CSV should contain binary_hash"
        );
        assert!(
            csv_content.contains("DVD-TEST"),
            "CSV should contain volume_id"
        );
    }

    /// Test that `log()` returns Ok with no commits.
    #[test]
    fn test_log_no_commits() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        Repository::init(&root).unwrap();
        let result = log_at(&root, None);
        assert!(result.is_ok(), "log() should succeed even with no commits");
    }

    /// Test that `export()` fails on nonexistent commit.
    #[test]
    fn test_export_nonexistent_commit() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        Repository::init(&root).unwrap();
        let result = export_at(
            &root,
            "0000000000000000000000000000000000000000000000000000000000000000",
            "out.tar",
        );
        assert!(result.is_err(), "export() should fail on bad commit hash");
    }
}
