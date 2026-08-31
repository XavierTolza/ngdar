use crate::error::NgdarError;
use std::path::{Path, PathBuf};

/// The `.ngdar` directory structure within a repository.
pub const NGDAR_DIR: &str = ".ngdar";
pub const OBJECTS_DIR: &str = "objects";
pub const HEAD_FILE: &str = "HEAD";
pub const INDEX_FILE: &str = "index";
pub const REPO_ID_FILE: &str = "repository_id";
pub const NGDAR_IGNORE_FILE: &str = ".ngdarignore";

/// Repository configuration, read from the `.ngdar/` directory.
pub struct Repository {
    pub path: PathBuf,
    pub ngdar_path: PathBuf,
    pub objects_path: PathBuf,
    pub head_path: PathBuf,
    pub index_path: PathBuf,
    #[allow(dead_code)]
    pub repo_id_path: PathBuf,
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
            repo_id_path,
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
            repo_id_path: ngdar_path.join(REPO_ID_FILE),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        (dir, root)
    }

    #[test]
    fn test_init_and_open() {
        let (_d, root) = setup();
        let repo = Repository::init(&root).unwrap();
        assert!(root.join(NGDAR_DIR).is_dir());
        assert!(root.join(NGDAR_DIR).join(REPO_ID_FILE).exists());
        assert!(root.join(NGDAR_DIR).join(INDEX_FILE).exists());
        assert!(root.join(NGDAR_DIR).join(HEAD_FILE).exists());
        assert!(root.join(NGDAR_DIR).join(OBJECTS_DIR).is_dir());
        assert!(!repo.repo_id.is_empty());

        // Re-open
        let repo2 = Repository::find(&root).unwrap();
        assert_eq!(repo.repo_id, repo2.repo_id);
    }

    #[test]
    fn test_find_from_subdir() {
        let (_d, root) = setup();
        Repository::init(&root).unwrap();
        let sub = root.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        let repo = Repository::find(&sub).unwrap();
        assert_eq!(repo.path, root);
    }

    #[test]
    fn test_read_write_head() {
        let (_d, root) = setup();
        let repo = Repository::init(&root).unwrap();
        assert!(repo.read_head().unwrap().is_none());
        repo.write_head("abc123").unwrap();
        assert_eq!(repo.read_head().unwrap().unwrap(), "abc123");
    }

    #[test]
    fn test_read_write_index() {
        let (_d, root) = setup();
        let repo = Repository::init(&root).unwrap();
        let entries = vec!["file1.txt".into(), "dir/file2.txt".into()];
        repo.write_index(&entries).unwrap();
        let read = repo.read_index().unwrap();
        assert_eq!(read, entries);
    }
}
