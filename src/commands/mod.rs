//! Command implementations — each subcommand has its own module.
//!
//! The [`crate::commands`] module re-exports all public command functions.
//! Shared helper utilities used by multiple commands live here.

use crate::config::Repository;
use crate::error::NgdarError;
use crate::hasher::{mtime_secs, HashProxy};
use crate::ignore::{self, IgnoreRules};
use crate::objects;
use indicatif::{ProgressBar, ProgressStyle};
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

#[cfg(unix)]
fn format_permissions(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn format_permissions(_metadata: &std::fs::Metadata) -> u32 {
    0o644
}

fn get_mtime(metadata: &std::fs::Metadata) -> i64 {
    mtime_secs(metadata)
}

/// Format a byte count as a human-readable string (B, KB, MB, GB).
fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KB", value / KIB)
    } else {
        format!("{} B", bytes)
    }
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
        let tar_path = format!("{}/{}", tar_prefix, crate::path_to_slash(relative));

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

/// Append a single regular file to the tar builder under its repository path.
fn append_file_to_tar(
    builder: &mut tar::Builder<&std::fs::File>,
    full_path: &Path,
    tar_path: &str,
) -> Result<(), NgdarError> {
    let file_data = std::fs::read(full_path)?;
    let metadata = std::fs::metadata(full_path)?;

    let mut header = tar::Header::new_gnu();
    header.set_size(file_data.len() as u64);
    header.set_mode(format_permissions(&metadata));
    header.set_mtime(get_mtime(&metadata) as u64);
    header.set_entry_type(tar::EntryType::Regular);

    builder
        .append_data(&mut header, tar_path, std::io::Cursor::new(&file_data))
        .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
    Ok(())
}

/// Verify and append every file referenced by a list of Meta objects.
///
/// Each entry is a `(meta_hash, rel_path)` pair. The referenced binary is read
/// from disk after checking that it still exists and that its BLAKE3 hash
/// matches the recorded `binary_hash`; missing or modified files are skipped
/// with a warning. Returns the number of files written and skipped.
///
/// `progress` is advanced by the size of each file that is written. When
/// `verbose` is set, each written file is listed (above the live progress bar
/// on a terminal, or as plain stdout when output is redirected).
fn append_meta_files_to_tar(
    builder: &mut tar::Builder<&std::fs::File>,
    repo: &Repository,
    meta_refs: &[(String, String)],
    progress: &ProgressBar,
    verbose: bool,
) -> Result<(u64, u64), NgdarError> {
    use std::io::IsTerminal;

    let is_tty = std::io::stdout().is_terminal();
    let hasher = HashProxy::load(&repo.path, &repo.cache_path())?;
    let mut written = 0u64;
    let mut warnings = 0u64;
    for (meta_hash, _rel_path) in meta_refs {
        let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
        let Ok(meta) = objects::Meta::from_text(&meta_text) else {
            continue;
        };
        let full_path = repo.path.join(&meta.path);
        if !full_path.exists() {
            eprintln!("Warning: '{}' not found on disk, skipping", meta.path);
            warnings += 1;
            continue;
        }
        // Verify on-disk content matches the archived hash. `compute` reads the
        // file and deliberately does not write the cache: recording a hash here
        // could hide an uncommitted modification from a later `status`.
        let actual_hex = hasher.compute(&meta.path)?;
        if actual_hex != meta.binary_hash {
            eprintln!(
                "Warning: '{}' has been modified (hash mismatch), skipping",
                meta.path
            );
            warnings += 1;
            continue;
        }
        let size = std::fs::metadata(&full_path)?.len();
        append_file_to_tar(builder, &full_path, &meta.path)?;
        if verbose {
            let line = format!("   adding: {} ({})", meta.path, format_size(size));
            if is_tty {
                // Keeps verbose output printed above the live progress bar.
                progress.println(line);
            } else {
                println!("{}", line);
            }
        }
        progress.inc(size);
        written += 1;
    }
    Ok((written, warnings))
}

/// Build a TAR archive containing the full `.ngdar/` metadata directory plus
/// the given Meta-referenced data files.
fn build_tar_archive(
    repo: &Repository,
    meta_refs: &[(String, String)],
    out_path: &str,
    total_bytes: u64,
    verbose: bool,
) -> Result<(u64, u64), NgdarError> {
    use std::io::IsTerminal;

    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);

    // --- Add full .ngdar/ metadata directory ---
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;

    // Progress is measured in bytes of data written to the archive. The bar is
    // only shown on a terminal; when output is redirected the verbose lines
    // below still go to plain stdout.
    let is_tty = std::io::stdout().is_terminal();
    let progress = if is_tty {
        let bar = ProgressBar::new(total_bytes);
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner} [{elapsed_precise}] [{bar:40}] {bytes}/{total_bytes} ({percent}%)",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=> "),
        );
        bar
    } else {
        ProgressBar::hidden()
    };

    // --- Add selected data files under their original paths ---
    let (written, warnings) =
        append_meta_files_to_tar(&mut builder, repo, meta_refs, &progress, verbose)?;

    progress.finish_and_clear();

    // Finalize the archive
    builder.finish()?;
    Ok((written, warnings))
}

/// Collect all `(meta_hash, rel_path)` pairs reachable from a commit's tree.
fn collect_commit_meta_refs(
    repo: &Repository,
    commit_hash: &str,
) -> Result<Vec<(String, String)>, NgdarError> {
    let commit_text = objects::read_object(&repo.objects_path, commit_hash)?;
    let commit = objects::Commit::from_text(&commit_text)?;
    let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
    let tree = objects::Tree::from_text(&tree_text)?;
    let mut meta_refs: Vec<(String, String)> = Vec::new();
    collect_meta_with_hashes(&repo.objects_path, &tree, &mut meta_refs, "")?;
    Ok(meta_refs)
}

/// Resolve the set of Meta references to package for a `pack` invocation.
///
/// - Single commit: every file reachable from that commit.
/// - Range `from..to`: every file added or changed between the two commits,
///   i.e. files whose Meta hash differs from the `from` endpoint (or which do
///   not exist there at all).
fn resolve_pack_meta_refs(
    repo: &Repository,
    from: &str,
    to: &str,
) -> Result<Vec<(String, String)>, NgdarError> {
    if from == to {
        return collect_commit_meta_refs(repo, to);
    }

    let old_refs = collect_commit_meta_refs(repo, from)?;
    let old_by_path: std::collections::HashMap<&str, &str> = old_refs
        .iter()
        .map(|(hash, path)| (path.as_str(), hash.as_str()))
        .collect();

    let new_refs = collect_commit_meta_refs(repo, to)?;
    let changed: Vec<(String, String)> = new_refs
        .into_iter()
        .filter(|(hash, path)| old_by_path.get(path.as_str()) != Some(&hash.as_str()))
        .collect();
    Ok(changed)
}

/// Resolve a commit identifier to a full commit hash.
///
/// Accepts a full 64-character hash or a unique prefix; prefixes are expanded
/// by scanning the commit history reachable from HEAD.
pub fn resolve_commit_id(repo: &Repository, id: &str) -> Result<String, NgdarError> {
    if id.len() == 64 && repo.objects_path.join(&id[..2]).join(&id[2..]).is_file() {
        return Ok(id.to_string());
    }

    let mut current = repo.read_head()?;
    while let Some(hash) = current {
        if hash.starts_with(id) {
            return Ok(hash);
        }
        let commit_text = objects::read_object(&repo.objects_path, &hash)?;
        let commit = objects::Commit::from_text(&commit_text)?;
        current = commit.parent_hash;
    }

    Err(NgdarError::Other(format!(
        "Commit '{}' not found in history",
        id
    )))
}

/// Resolve a volume identifier to the newest commit whose tree references a
/// Meta object carrying that `volume_id`.
pub fn resolve_volume_id(repo: &Repository, vol_id: &str) -> Result<String, NgdarError> {
    let mut current = repo.read_head()?;
    while let Some(hash) = current {
        let meta_refs = collect_commit_meta_refs(repo, &hash)?;
        let mut found = false;
        for (meta_hash, _path) in &meta_refs {
            let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
            if let Ok(meta) = objects::Meta::from_text(&meta_text) {
                if meta.volume_id == vol_id {
                    found = true;
                    break;
                }
            }
        }
        if found {
            return Ok(hash);
        }
        let commit_text = objects::read_object(&repo.objects_path, &hash)?;
        let commit = objects::Commit::from_text(&commit_text)?;
        current = commit.parent_hash;
    }

    Err(NgdarError::Other(format!(
        "Volume '{}' not found in history",
        vol_id
    )))
}

// ---------------------------------------------------------------------------
// Command submodules — each exposes a single `pub fn` for its verb.
// ---------------------------------------------------------------------------
/// `ngdar add <paths>` — stage files for the next archive.
pub mod add;
/// `ngdar commit --vol-id <ID> -m <msg>` — record staged files as a commit.
pub mod commit;
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
/// `ngdar pack <target> --out <file.tar>` — create archive for a commit/range.
pub mod pack;
/// `ngdar remove-commit <hash|HEAD>` — remove a commit from the history.
pub mod remove_commit;
/// `ngdar status` — show staged, unstaged, untracked files.
pub mod status;

pub use add::add;
pub use commit::commit;
pub use db_export::db_export;
pub use export::export;
pub use hash::hash;
pub use init::init;
pub use log::log;
pub use pack::pack;
pub use remove_commit::remove_commit;
pub use status::status;
