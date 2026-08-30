use crate::error::NgdarError;
use std::path::{Path, PathBuf};

/// A cache entry mapping file metadata to its BLAKE3 hash.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    pub size: u64,
    pub mtime: i64,
    pub hash: String,
    pub path: String,
}

/// The XDG cache store for file hashes.
pub struct CacheStore {
    pub path: PathBuf,
    entries: Vec<CacheEntry>,
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
    pub fn lookup(&self, size: u64, mtime: i64, rel_path: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.size == size && e.mtime == mtime && e.path == rel_path)
            .map(|e| e.hash.as_str())
    }

    /// Insert or update an entry.
    pub fn insert(&mut self, size: u64, mtime: i64, hash: String, rel_path: String) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.path == rel_path)
        {
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

    /// Save the cache to disk.
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_empty_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.tsv");
        let cache = CacheStore::load(&path).unwrap();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn test_cache_insert_and_lookup() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.tsv");
        let mut cache = CacheStore::load(&path).unwrap();
        cache.insert(1024, 1700000000, "abc123".into(), "file.txt".into());
        assert_eq!(cache.lookup(1024, 1700000000, "file.txt"), Some("abc123"));
        assert!(cache.lookup(2048, 1700000000, "file.txt").is_none());
    }

    #[test]
    fn test_cache_persistence() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.tsv");
        {
            let mut cache = CacheStore::load(&path).unwrap();
            cache.insert(512, 1700000001, "def456".into(), "doc.pdf".into());
            cache.save().unwrap();
        }
        {
            let cache = CacheStore::load(&path).unwrap();
            assert_eq!(cache.lookup(512, 1700000001, "doc.pdf"), Some("def456"));
        }
    }

    #[test]
    fn test_cache_update() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.tsv");
        let mut cache = CacheStore::load(&path).unwrap();
        cache.insert(100, 1000, "old".into(), "f.txt".into());
        cache.insert(200, 2000, "new".into(), "f.txt".into());
        assert_eq!(cache.lookup(200, 2000, "f.txt"), Some("new"));
        assert!(cache.lookup(100, 1000, "f.txt").is_none());
    }
}