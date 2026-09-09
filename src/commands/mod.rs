//! Command implementations — each subcommand has its own module.
//!
//! The [`crate::commands`] module re-exports all public command functions.
//! Shared helper utilities used by multiple commands live here.

use crate::cache::CacheStore;
use crate::config::Repository;
use crate::error::NgdarError;
use crate::hash::{hash_file, hash_to_hex};
use crate::ignore::{self, IgnoreRules};
use crate::objects;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn os_string() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    format!("{} {}", os, arch)
}

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

fn get_mtime(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

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

        // The .ngdar/deleted marker is added explicitly by the caller so that
        // it always reflects the exact commit being packed/exported rather
        // than whatever is currently on disk.
        if relative == crate::config::DELETED_FILE {
            continue;
        }

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

/// Append a `.ngdar/deleted` marker file to the TAR with the given paths.
fn append_deleted_to_tar(
    builder: &mut tar::Builder<&std::fs::File>,
    deleted: &[String],
) -> Result<(), NgdarError> {
    if deleted.is_empty() {
        return Ok(());
    }
    let deleted_content = deleted.join("\n");
    let mut header = tar::Header::new_gnu();
    header.set_size(deleted_content.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    let tar_path = format!(".ngdar/{}", crate::config::DELETED_FILE);
    builder
        .append_data(
            &mut header,
            tar_path.as_str(),
            std::io::Cursor::new(deleted_content),
        )
        .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))
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

fn build_tar_archive(
    repo: &Repository,
    staged_files: &[String],
    deleted: &[String],
    out_path: &str,
    _commit_hash: &str,
    _vol_id: &str,
) -> Result<(), NgdarError> {
    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);

    // --- Add full .ngdar/ metadata directory ---
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;

    // --- Add .ngdar/deleted marker for incremental extraction ---
    append_deleted_to_tar(&mut builder, deleted)?;

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

/// Split raw index entries into staged (added) and staged-deleted paths.
///
/// Index lines starting with `- ` mark files staged for deletion; all other
/// non-empty entries are treated as files staged for addition or modification.
pub fn parse_index(index: &[String]) -> (Vec<String>, Vec<String>) {
    let mut added = Vec::new();
    let mut deleted = Vec::new();
    for entry in index {
        if let Some(rest) = entry.strip_prefix(crate::config::DELETION_PREFIX) {
            deleted.push(rest.to_string());
        } else {
            added.push(entry.clone());
        }
    }
    (added, deleted)
}

// ---------------------------------------------------------------------------
// Command submodules — each exposes a single `pub fn` for its verb.
// ---------------------------------------------------------------------------
/// `ngdar add <paths>` — stage files for the next archive.
pub mod add;
/// `ngdar db-export <csv>` — export object database as CSV.
pub mod db_export;
/// `ngdar export <commit> <out>` — extract files from a commit.
pub mod export;
/// `ngdar hash <file>` — print BLAKE3 hash of a file.
pub mod hash;
/// `ngdar init` — initialise a new repository.
pub mod init;
/// `ngdar log [commit]` — show commit history.
pub mod log;
/// `ngdar pack --vol-id <ID> --out <file.tar> -m <msg>` — create archive.
pub mod pack;
/// `ngdar status` — show staged, unstaged, untracked files.
pub mod status;

pub use add::add;
pub use db_export::db_export;
pub use export::export;
pub use hash::hash;
pub use init::init;
pub use log::log;
pub use pack::pack;
pub use status::status;
