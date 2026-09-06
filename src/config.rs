use crate::error::NgdarError;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The `.ngdar` directory structure within a repository.
pub const NGDAR_DIR: &str = ".ngdar";
/// The objects subdirectory name (`.ngdar/objects`).
pub const OBJECTS_DIR: &str = "objects";
/// The HEAD file name (`.ngdar/HEAD`), containing the latest commit hash.
pub const HEAD_FILE: &str = "HEAD";
/// The index file name (`.ngdar/index`), containing the staging area.
pub const INDEX_FILE: &str = "index";
/// The repository ID file name (`.ngdar/repository_id`), containing a UUID v4.
pub const REPO_ID_FILE: &str = "repository_id";
/// The ignore file name (`.ngdarignore`) at the repository root.
pub const NGDAR_IGNORE_FILE: &str = ".ngdarignore";
/// The committed tracking file (`.ngdar/committed`), mapping `binary_hash path`.
pub const COMMITTED_FILE: &str = "committed";

/// Repository configuration, read from the `.ngdar/` directory.
pub struct Repository {
    /// Absolute path to the repository root.
    pub path: PathBuf,
    /// Absolute path to `.ngdar/` directory.
    pub ngdar_path: PathBuf,
    /// Absolute path to `.ngdar/objects/` directory.
    pub objects_path: PathBuf,
    /// Absolute path to `.ngdar/HEAD` file.
    pub head_path: PathBuf,
    /// Absolute path to `.ngdar/index` file.
    pub index_path: PathBuf,
    /// Absolute path to `.ngdar/committed` file.
    pub committed_path: PathBuf,
    /// Unique repository identifier (UUID v4).
    pub repo_id: String,
}

impl Repository {
    /// Find the `.ngdar` directory starting from `cwd` and walking up.
    pub fn find(cwd: &Path) -> Result<Repository, NgdarError> {
        let mut current = Some(cwd.to_path_buf());
        while let Some(dir) = current {
            let ngdar_path = dir.join(NGDAR_DIR);
            if ngdar_path.is_dir() {
                return Self::open(&dir);
            }
            current = dir.parent().map(|p| p.to_path_buf());
        }
        Err(NgdarError::NotARepository)
    }

    /// Open an existing repository at the given root.
    pub fn open(root: &Path) -> Result<Repository, NgdarError> {
        let ngdar_path = root.join(NGDAR_DIR);
        if !ngdar_path.is_dir() {
            return Err(NgdarError::NotARepository);
        }
        let repo_id_path = ngdar_path.join(REPO_ID_FILE);
        let repo_id = std::fs::read_to_string(&repo_id_path)
            .map_err(|_| NgdarError::Config("Cannot read repository_id".into()))?
            .trim()
            .to_string();

        Ok(Repository {
            path: root.to_path_buf(),
            objects_path: ngdar_path.join(OBJECTS_DIR),
            head_path: ngdar_path.join(HEAD_FILE),
            index_path: ngdar_path.join(INDEX_FILE),
            committed_path: ngdar_path.join(COMMITTED_FILE),
            ngdar_path,
            repo_id,
        })
    }

    /// Initialize a new repository at the given root.
    pub fn init(root: &Path) -> Result<Repository, NgdarError> {
        if root.join(NGDAR_DIR).exists() {
            return Err(NgdarError::Config("Repository already initialized".into()));
        }

        let repo_id = uuid::Uuid::new_v4().to_string();
        let ngdar_path = root.join(NGDAR_DIR);
        let objects_path = ngdar_path.join(OBJECTS_DIR);

        std::fs::create_dir_all(&ngdar_path)?;
        std::fs::create_dir_all(&objects_path)?;

        // Write repository_id
        std::fs::write(ngdar_path.join(REPO_ID_FILE), &repo_id)?;

        // Create empty index
        std::fs::write(ngdar_path.join(INDEX_FILE), "")?;

        // Create HEAD pointing to nothing yet
        std::fs::write(ngdar_path.join(HEAD_FILE), "")?;

        Ok(Repository {
            path: root.to_path_buf(),
            objects_path,
            head_path: ngdar_path.join(HEAD_FILE),
            index_path: ngdar_path.join(INDEX_FILE),
            committed_path: ngdar_path.join(COMMITTED_FILE),
            ngdar_path,
            repo_id,
        })
    }

    /// Get the cache directory path for this repository.
    pub fn cache_dir(&self) -> PathBuf {
        let base = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".cache")
            });
        base.join("ngdar").join(&self.repo_id)
    }

    /// Get the cache file path.
    pub fn cache_path(&self) -> PathBuf {
        self.cache_dir().join("cache.tsv")
    }

    /// Read the current HEAD pointer.
    pub fn read_head(&self) -> Result<Option<String>, NgdarError> {
        let content = std::fs::read_to_string(&self.head_path)?;
        let trimmed = content.trim().to_string();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed))
        }
    }

    /// Write the HEAD pointer.
    pub fn write_head(&self, hash: &str) -> Result<(), NgdarError> {
        std::fs::write(&self.head_path, hash)?;
        Ok(())
    }

    /// Read the current index (staging area).
    pub fn read_index(&self) -> Result<Vec<String>, NgdarError> {
        let content = std::fs::read_to_string(&self.index_path)?;
        Ok(content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Write the index.
    pub fn write_index(&self, entries: &[String]) -> Result<(), NgdarError> {
        std::fs::write(&self.index_path, entries.join("\n"))?;
        Ok(())
    }

    /// Clear the index.
    pub fn clear_index(&self) -> Result<(), NgdarError> {
        std::fs::write(&self.index_path, "")?;
        Ok(())
    }

    /// Read the committed file — returns `Vec<(binary_hash, path)>`.
    pub fn read_committed(&self) -> Result<Vec<(String, String)>, NgdarError> {
        if !self.committed_path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.committed_path)?;
        Ok(content
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                if l.is_empty() {
                    return None;
                }
                let (hash, path) = l.split_once(' ')?;
                Some((hash.to_string(), path.to_string()))
            })
            .collect())
    }

    /// Append entries to the committed file (one `hash path` per line).
    pub fn add_committed(&self, entries: &[(String, String)]) -> Result<(), NgdarError> {
        let mut content = String::new();
        for (hash, path) in entries {
            content.push_str(&format!("{} {}\n", hash, path));
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.committed_path)?
            .write_all(content.as_bytes())?;
        Ok(())
    }
}
