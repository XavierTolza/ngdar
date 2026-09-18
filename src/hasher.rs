//! Hash proxy — the single entry point for obtaining a tracked file's BLAKE3
//! hash.
//!
//! Every command that needs the hash of a file inside the repository goes
//! through [`HashProxy`](crate::hasher::HashProxy) so hashing behaviour is
//! identical everywhere: the file is fingerprinted by `(size, mtime)`, and if
//! that fingerprint matches the cached one the cached hash is returned without
//! re-reading the file. Otherwise the file is hashed and the cache is updated.
//!
//! Entries are keyed by repository-relative path in a hash map so lookups stay
//! O(1) even for repositories with a very large number of files.

use crate::cache::{CacheEntry, CacheStore};
use crate::error::NgdarError;
use crate::hash::{hash_file, hash_to_hex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Modification time of a file, in whole seconds since the Unix epoch.
///
/// Used for the `mtime` recorded in Meta objects and TAR headers.
pub fn mtime_secs(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Modification time of a file, in nanoseconds since the Unix epoch.
///
/// Used as the cache fingerprint: sub-second resolution is what lets the proxy
/// notice a file that was rewritten within the same second with an identical
/// size.
fn mtime_nanos(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

/// Cache-aware accessor for file hashes within a repository.
///
/// Load one per command run with [`HashProxy::load`], call [`HashProxy::hash`]
/// for each file, then persist with [`HashProxy::save`].
pub struct HashProxy {
    root: PathBuf,
    cache_path: PathBuf,
    entries: HashMap<String, CacheEntry>,
}

impl HashProxy {
    /// Load the proxy for `root`, reading existing fingerprints from
    /// `cache_path` (creating an empty set if the file does not exist yet).
    pub fn load(root: &Path, cache_path: &Path) -> Result<Self, NgdarError> {
        let store = CacheStore::load(cache_path)?;
        let entries = store
            .entries
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect();
        Ok(HashProxy {
            root: root.to_path_buf(),
            cache_path: cache_path.to_path_buf(),
            entries,
        })
    }

    /// Return the BLAKE3 hex hash of the file at `rel_path`.
    ///
    /// The hash is served from the cache when the file's `(size, mtime)`
    /// fingerprint is unchanged; otherwise the file is re-hashed and the cache
    /// is updated.
    pub fn hash(&mut self, rel_path: &str) -> Result<String, NgdarError> {
        let full_path = self.root.join(rel_path);
        let metadata = std::fs::metadata(&full_path)?;
        let size = metadata.len();
        let secs = mtime_secs(&metadata);
        let nanos = mtime_nanos(&metadata);

        if let Some(entry) = self.entries.get(rel_path) {
            if entry.size == size && (entry.mtime == nanos || entry.mtime == secs) {
                let cached = entry.hash.clone();
                let legacy = entry.mtime != nanos;
                // Refresh legacy second-resolution entries to the nanosecond
                // fingerprint so future runs can detect same-second rewrites.
                if legacy {
                    self.entries.insert(
                        rel_path.to_string(),
                        CacheEntry {
                            size,
                            mtime: nanos,
                            hash: cached.clone(),
                            path: rel_path.to_string(),
                        },
                    );
                }
                return Ok(cached);
            }
        }

        let hex = hash_to_hex(&hash_file(&full_path)?);
        self.entries.insert(
            rel_path.to_string(),
            CacheEntry {
                size,
                mtime: nanos,
                hash: hex.clone(),
                path: rel_path.to_string(),
            },
        );
        Ok(hex)
    }

    /// Return the cached hash of `rel_path` if the file still matches its cached
    /// `(size, mtime)` fingerprint, or `None` when the file is missing or has
    /// changed. This never reads the file contents.
    pub fn cached_hash(&self, rel_path: &str) -> Option<&str> {
        let full_path = self.root.join(rel_path);
        let metadata = std::fs::metadata(&full_path).ok()?;
        let entry = self.entries.get(rel_path)?;
        let matches = entry.size == metadata.len()
            && (entry.mtime == mtime_nanos(&metadata) || entry.mtime == mtime_secs(&metadata));
        if matches {
            Some(entry.hash.as_str())
        } else {
            None
        }
    }

    /// Compute the BLAKE3 hex hash of `rel_path` by reading the file, ignoring
    /// the cache entirely — neither reading from it nor writing to it.
    ///
    /// Used where the on-disk content must be verified rather than trusted: the
    /// pack/export integrity check, and `status` when it needs to decide whether
    /// a cache-missed file really changed.
    pub fn compute(&self, rel_path: &str) -> Result<String, NgdarError> {
        Ok(hash_to_hex(&hash_file(&self.root.join(rel_path))?))
    }

    /// Record `hash` for `rel_path` under its current `(size, mtime)`
    /// fingerprint, without reading the file contents back.
    pub fn record(&mut self, rel_path: &str, hash: String) -> Result<(), NgdarError> {
        let metadata = std::fs::metadata(self.root.join(rel_path))?;
        self.entries.insert(
            rel_path.to_string(),
            CacheEntry {
                size: metadata.len(),
                mtime: mtime_nanos(&metadata),
                hash,
                path: rel_path.to_string(),
            },
        );
        Ok(())
    }

    /// Persist the fingerprints to disk.
    pub fn save(&self) -> Result<(), NgdarError> {
        let store = CacheStore {
            path: self.cache_path.clone(),
            entries: self.entries.values().cloned().collect(),
        };
        store.save()
    }
}
