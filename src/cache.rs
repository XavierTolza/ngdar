//! XDG-compatible file hash cache.
//!
//! Stores BLAKE3 hashes keyed by (size, mtime, path) to avoid re-hashing
//! unchanged files between `ngdar` invocations.
//!
//! The cache is stored in `~/.cache/ngdar/<repo-id>/cache.tsv` as a TSV file.

use crate::error::NgdarError;
use std::path::{Path, PathBuf};

/// A single cache entry mapping file metadata to its BLAKE3 hash.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    /// File size in bytes.
    pub size: u64,
    /// File modification time (Unix timestamp, seconds since epoch).
    pub mtime: i64,
    /// BLAKE3 hash of the file contents (64-char hex string).
    pub hash: String,
    /// Relative path of the file within the repository.
    pub path: String,
}

/// The XDG cache store for file hashes.
///
/// Loads from and saves to a TSV file at the configured cache path.
pub struct CacheStore {
    /// Path to the cache TSV file on disk.
    pub path: PathBuf,
    /// In-memory cache entries.
    pub entries: Vec<CacheEntry>,
}

impl CacheStore {
    /// Load the cache from disk. Creates an empty cache if the file doesn't exist.
    pub fn load(cache_path: &Path) -> Result<Self, NgdarError> {
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let entries = if cache_path.exists() {
            let content = std::fs::read_to_string(cache_path)?;
            content
                .lines()
                .filter_map(|line| {
                    let line = line.trim();
                    if line.is_empty() {
                        return None;
                    }
                    let parts: Vec<&str> = line.splitn(4, '\t').collect();
                    if parts.len() < 4 {
                        return None;
                    }
                    Some(CacheEntry {
                        size: parts[0].parse().ok()?,
                        mtime: parts[1].parse().ok()?,
                        hash: parts[2].to_string(),
                        path: parts[3].to_string(),
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(CacheStore {
            path: cache_path.to_path_buf(),
            entries,
        })
    }

    /// Look up a file in the cache by its size and mtime.
    ///
    /// Returns the cached hash if an entry with matching (size, mtime, path)
    /// exists, or `None` otherwise.
    pub fn lookup(&self, size: u64, mtime: i64, rel_path: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.size == size && e.mtime == mtime && e.path == rel_path)
            .map(|e| e.hash.as_str())
    }

    /// Insert or update an entry in the cache.
    ///
    /// If an entry for `rel_path` already exists, its size, mtime, and hash
    /// are updated. Otherwise a new entry is appended.
    pub fn insert(&mut self, size: u64, mtime: i64, hash: String, rel_path: String) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == rel_path) {
            existing.size = size;
            existing.mtime = mtime;
            existing.hash = hash;
        } else {
            self.entries.push(CacheEntry {
                size,
                mtime,
                hash,
                path: rel_path,
            });
        }
    }

    /// Save the cache to disk as a TSV file.
    pub fn save(&self) -> Result<(), NgdarError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for entry in &self.entries {
            content.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                entry.size, entry.mtime, entry.hash, entry.path
            ));
        }
        std::fs::write(&self.path, content)?;
        Ok(())
    }
}
