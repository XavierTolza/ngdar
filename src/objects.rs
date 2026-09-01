use crate::error::NgdarError;
use crate::hash::{hash_string, hash_to_hex};
use std::path::Path;

/// ---------------------------------------------------------------------------
/// Object storage — each object is stored as a text file named by its BLAKE3
/// hash in `.ngdar/objects/<first-two-chars>/<rest-of-hash>`.
/// ---------------------------------------------------------------------------

/// Compute the storage path for an object given its hex hash.
///
/// Objects are sharded into subdirectories by the first two hex characters
/// to avoid excessive directory entries (e.g., `objects/ab/cdef...`).
pub fn object_path(objects_dir: &Path, hex_hash: &str) -> std::path::PathBuf {
    let (prefix, rest) = hex_hash.split_at(2);
    objects_dir.join(prefix).join(rest)
}

/// Write an object to the store, returning its BLAKE3 hex hash.
///
/// The content is hashed with BLAKE3 to determine the storage path.
/// If the object already exists, it is not overwritten (content-addressable
/// storage guarantees deduplication).
pub fn write_object(objects_dir: &Path, content: &str) -> Result<String, NgdarError> {
    let hash = hash_string(content);
    let hex = hash_to_hex(&hash);
    let path = object_path(objects_dir, &hex);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::write(&path, content)?;
    }
    Ok(hex)
}

/// Read an object from the store by its hex hash.
///
/// Returns the object's text content. Returns an error if the object file
/// does not exist.
pub fn read_object(objects_dir: &Path, hex_hash: &str) -> Result<String, NgdarError> {
    let path = object_path(objects_dir, hex_hash);
    std::fs::read_to_string(&path)
        .map_err(|e| NgdarError::Other(format!("Object not found {}: {}", hex_hash, e)))
}

/// ---------------------------------------------------------------------------
/// Meta object — represents a file's metadata and its physical volume location.
/// Does NOT contain the binary data.
/// ---------------------------------------------------------------------------

/// Metadata about a tracked file.
///
/// Stores file size, modification time, permissions, the BLAKE3 hash of the
/// file's binary content, and the volume ID of the archive where the binary
/// was stored.
#[derive(Debug, Clone)]
pub struct Meta {
    /// File size in bytes.
    pub size: u64,
    /// File modification time (Unix timestamp).
    pub mtime: i64,
    /// Unix file permissions (e.g., `0o644`).
    pub permissions: u32,
    /// BLAKE3 hash of the file's binary content.
    pub binary_hash: String,
    /// Volume identifier (e.g., "DVD-001", "ARCHIVE-2026-08").
    pub volume_id: String,
}

impl Meta {
    /// Create a new Meta object.
    pub fn new(
        size: u64,
        mtime: i64,
        permissions: u32,
        binary_hash: String,
        volume_id: String,
    ) -> Self {
        Meta {
            size,
            mtime,
            permissions,
            binary_hash,
            volume_id,
        }
    }

    /// Serialize to the text format used in object storage.
    ///
    /// Format:
    /// ```text
    /// type meta
    /// size <bytes>
    /// mtime <unix_ts>
    /// permissions <octal>
    /// binary_hash <blake3_hex>
    /// volume_id <id>
    /// ```
    pub fn to_text(&self) -> String {
        format!(
            "type meta\n\
             size {}\n\
             mtime {}\n\
             permissions {:o}\n\
             binary_hash {}\n\
             volume_id {}\n",
            self.size, self.mtime, self.permissions, self.binary_hash, self.volume_id
        )
    }

    /// Parse a Meta object from its text representation.
    pub(crate) fn from_text(text: &str) -> Result<Self, NgdarError> {
        let mut size = None;
        let mut mtime = None;
        let mut permissions = None;
        let mut binary_hash = None;
        let mut volume_id = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("type") {
                continue;
            }
            if let Some((key, value)) = line.split_once(' ') {
                match key {
                    "size" => {
                        size = Some(
                            value
                                .parse::<u64>()
                                .map_err(|_| NgdarError::Parse("size".into()))?,
                        )
                    }
                    "mtime" => {
                        mtime = Some(
                            value
                                .parse::<i64>()
                                .map_err(|_| NgdarError::Parse("mtime".into()))?,
                        )
                    }
                    "permissions" => {
                        permissions = Some(
                            u32::from_str_radix(value, 8)
                                .map_err(|_| NgdarError::Parse("permissions".into()))?,
                        )
                    }
                    "binary_hash" => binary_hash = Some(value.to_string()),
                    "volume_id" => volume_id = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Ok(Meta {
            size: size.ok_or_else(|| NgdarError::Parse("Missing size".into()))?,
            mtime: mtime.ok_or_else(|| NgdarError::Parse("Missing mtime".into()))?,
            permissions: permissions
                .ok_or_else(|| NgdarError::Parse("Missing permissions".into()))?,
            binary_hash: binary_hash
                .ok_or_else(|| NgdarError::Parse("Missing binary_hash".into()))?,
            volume_id: volume_id.ok_or_else(|| NgdarError::Parse("Missing volume_id".into()))?,
        })
    }
}

/// ---------------------------------------------------------------------------
/// Tree object — a snapshot of a directory at a point in time.
/// Each entry is either a `"tree"` (referencing a subdirectory Tree object)
/// or a `"meta"` (referencing a Meta object). Entries are sorted by name.
/// ---------------------------------------------------------------------------

/// A single entry in a Tree object.
#[derive(Debug, Clone)]
pub struct TreeEntry {
    /// Entry kind: `"tree"` for subdirectories, `"meta"` for files.
    pub kind: String,
    /// BLAKE3 hash of the referenced object (Tree or Meta).
    pub hash: String,
    /// File or directory name (not a full path).
    pub name: String,
}

/// A Tree object — a snapshot of a directory at a point in time.
///
/// Each entry is either a `"tree"` (referencing a subdirectory Tree object)
/// or a `"meta"` (referencing a Meta object). Entries are sorted by name.
#[derive(Debug, Clone)]
pub struct Tree {
    /// Entries in this tree, sorted by name.
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    /// Create a new empty Tree.
    pub fn new() -> Self {
        Tree {
            entries: Vec::new(),
        }
    }

    /// Add an entry to the tree, maintaining sort order by name.
    pub fn add(&mut self, kind: &str, hash: String, name: String) {
        self.entries.push(TreeEntry {
            kind: kind.to_string(),
            hash,
            name,
        });
        self.entries.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Serialize to the text format.
    ///
    /// Format: one `<kind> <hash> <name>` per line.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for entry in &self.entries {
            s.push_str(&format!("{} {} {}\n", entry.kind, entry.hash, entry.name));
        }
        s
    }

    /// Parse a Tree object from its text representation.
    pub fn from_text(text: &str) -> Result<Self, NgdarError> {
        let mut entries = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(3, ' ').collect();
            if parts.len() < 3 {
                return Err(NgdarError::Parse(format!("Invalid tree line: {}", line)));
            }
            entries.push(TreeEntry {
                kind: parts[0].to_string(),
                hash: parts[1].to_string(),
                name: parts[2].to_string(),
            });
        }
        Ok(Tree { entries })
    }
}

/// ---------------------------------------------------------------------------
/// Commit object — a snapshot of the entire repository state.
/// ---------------------------------------------------------------------------

/// A Commit object — a snapshot of the entire repository state.
///
/// Contains a reference to the root Tree object, an optional parent commit,
/// authorship and provenance metadata, a timestamp, and a commit message.
#[derive(Debug, Clone)]
pub struct Commit {
    /// BLAKE3 hash of the root Tree object.
    pub tree_hash: String,
    /// Optional hash of the parent commit (`None` for the first commit).
    pub parent_hash: Option<String>,
    /// Author string (e.g., `"user <user@host>"`).
    pub author: String,
    /// Operating system string (e.g., `"Linux x86_64"`).
    pub os: String,
    /// Tool version (e.g., `"ngdar-1.0.0"`).
    pub tool_version: String,
    /// Commit timestamp (Unix seconds since epoch).
    pub timestamp: i64,
    /// Commit message.
    pub message: String,
}

impl Commit {
    /// Create a new Commit object.
    pub fn new(
        tree_hash: String,
        parent_hash: Option<String>,
        author: String,
        os: String,
        tool_version: String,
        timestamp: i64,
        message: String,
    ) -> Self {
        Commit {
            tree_hash,
            parent_hash,
            author,
            os,
            tool_version,
            timestamp,
            message,
        }
    }

    /// Serialize to the text format used in object storage.
    ///
    /// Format:
    /// ```text
    /// tree <hash>
    /// parent <hash-or-none>
    /// author <string>
    /// os <string>
    /// tool_version <string>
    /// timestamp <unix_ts>
    ///
    /// <message>
    /// ```
    pub fn to_text(&self) -> String {
        let parent = match &self.parent_hash {
            Some(hash) => format!("parent {}", hash),
            None => "parent none".to_string(),
        };
        format!(
            "tree {}\n\
             {}\n\
             author {}\n\
             os {}\n\
             tool_version {}\n\
             timestamp {}\n\
             \n\
             {}\n",
            self.tree_hash,
            parent,
            self.author,
            self.os,
            self.tool_version,
            self.timestamp,
            self.message
        )
    }

    /// Parse a Commit object from its text representation.
    pub fn from_text(text: &str) -> Result<Self, NgdarError> {
        let mut tree_hash = None;
        let mut parent_hash = None;
        let mut author = None;
        let mut os = None;
        let mut tool_version = None;
        let mut timestamp = None;
        let mut message = String::new();
        let mut in_message = false;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() && !in_message {
                in_message = true;
                continue;
            }
            if in_message {
                if !message.is_empty() {
                    message.push('\n');
                }
                message.push_str(line);
                continue;
            }
            if let Some((key, value)) = line.split_once(' ') {
                match key {
                    "tree" => tree_hash = Some(value.to_string()),
                    "parent" => {
                        if value != "none" {
                            parent_hash = Some(value.to_string());
                        }
                    }
                    "author" => author = Some(value.to_string()),
                    "os" => os = Some(value.to_string()),
                    "tool_version" => tool_version = Some(value.to_string()),
                    "timestamp" => {
                        timestamp = Some(
                            value
                                .parse::<i64>()
                                .map_err(|_| NgdarError::Parse("timestamp".into()))?,
                        )
                    }
                    _ => {}
                }
            }
        }

        Ok(Commit {
            tree_hash: tree_hash.ok_or_else(|| NgdarError::Parse("Missing tree".into()))?,
            parent_hash,
            author: author.ok_or_else(|| NgdarError::Parse("Missing author".into()))?,
            os: os.ok_or_else(|| NgdarError::Parse("Missing os".into()))?,
            tool_version: tool_version
                .ok_or_else(|| NgdarError::Parse("Missing tool_version".into()))?,
            timestamp: timestamp.ok_or_else(|| NgdarError::Parse("Missing timestamp".into()))?,
            message,
        })
    }
}

/// ---------------------------------------------------------------------------
/// High-level helpers for building tree objects from index entries.
/// ---------------------------------------------------------------------------

/// Build a tree structure from a list of staged (indexed) files.
///
/// Takes a list of `(rel_path, meta_hash)` pairs and constructs a
/// hierarchical tree of Tree objects in the object store. Returns the
/// root tree hash and a list of `(meta_hash, rel_path)` for all leaf files.
///
/// Directories are created bottom-up: subdirectory tree objects are built
/// first and then referenced by their parent directory tree objects.
pub fn build_tree_from_index(
    objects_dir: &Path,
    staged_files: &[(&str, &str)], // (rel_path, meta_hash) pairs
) -> Result<(String, Vec<(String, String)>), NgdarError> {
    // Group by directory
    let mut dirs: std::collections::BTreeMap<String, Vec<TreeEntry>> =
        std::collections::BTreeMap::new();

    for (rel_path, meta_hash) in staged_files {
        let path = std::path::Path::new(rel_path);
        let parent = path
            .parent()
            .map(|p| p.to_str().unwrap_or(""))
            .unwrap_or("");
        let filename = path.file_name().unwrap().to_str().unwrap_or("");

        let entry = TreeEntry {
            kind: "meta".to_string(),
            hash: (*meta_hash).to_string(),
            name: filename.to_string(),
        };

        dirs.entry(parent.to_string()).or_default().push(entry);
    }

    // Build trees bottom-up
    let mut tree_cache: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    // Get all unique directory paths sorted longest-first (bottom-up)
    let mut dir_paths: Vec<String> = dirs.keys().cloned().collect();
    dir_paths.sort_by_key(|a| std::cmp::Reverse(a.len())); // reverse sort

    for dir_path in &dir_paths {
        let mut tree = Tree::new();
        if let Some(entries) = dirs.get(dir_path) {
            for entry in entries {
                tree.add(&entry.kind, entry.hash.clone(), entry.name.clone());
            }
        }
        // Also add subdirectory trees that we already built
        let prefix = if dir_path.is_empty() {
            String::new()
        } else {
            format!("{}/", dir_path)
        };
        for (sub_path, sub_hash) in &tree_cache {
            if let Some(rest) = sub_path.strip_prefix(&prefix) {
                if !rest.contains('/') {
                    // Direct child
                    tree.add("tree", sub_hash.clone(), rest.to_string());
                }
            }
        }

        let text = tree.to_text();
        let hash = write_object(objects_dir, &text)?;
        tree_cache.insert(dir_path.clone(), hash);
    }

    // Collect (meta_hash, rel_path) pairs for all leaf files
    let leaf_files: Vec<(String, String)> = staged_files
        .iter()
        .map(|(path, hash)| (hash.to_string(), path.to_string()))
        .collect();

    let root_hash = tree_cache
        .get("")
        .cloned()
        .or_else(|| tree_cache.get(".").cloned())
        .unwrap_or_else(|| {
            // Empty tree
            let text = Tree::new().to_text();
            write_object(objects_dir, &text).unwrap_or_default()
        });

    Ok((root_hash, leaf_files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_roundtrip() {
        let meta = Meta::new(
            1048576,
            1788118000,
            0o644,
            "a1b2c3d4e5f6".into(),
            "DVD-001".into(),
        );
        let text = meta.to_text();
        let parsed = Meta::from_text(&text).unwrap();
        assert_eq!(parsed.size, 1048576);
        assert_eq!(parsed.mtime, 1788118000);
        assert_eq!(parsed.permissions, 0o644);
        assert_eq!(parsed.binary_hash, "a1b2c3d4e5f6");
        assert_eq!(parsed.volume_id, "DVD-001");
    }

    #[test]
    fn test_tree_roundtrip() {
        let mut tree = Tree::new();
        tree.add("meta", "abc123".into(), "file.txt".into());
        tree.add("tree", "def456".into(), "subdir".into());
        let text = tree.to_text();
        let parsed = Tree::from_text(&text).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0].name, "file.txt");
        assert_eq!(parsed.entries[1].name, "subdir");
    }

    #[test]
    fn test_commit_roundtrip() {
        let commit = Commit::new(
            "treehash123".into(),
            Some("parent456".into()),
            "Xavier".into(),
            "Linux 6.6.0-x86_64".into(),
            "ngdar-1.0.0".into(),
            1788118091,
            "Test commit message.".into(),
        );
        let text = commit.to_text();
        let parsed = Commit::from_text(&text).unwrap();
        assert_eq!(parsed.tree_hash, "treehash123");
        assert_eq!(parsed.parent_hash.unwrap(), "parent456");
        assert_eq!(parsed.message, "Test commit message.");
    }

    #[test]
    fn test_commit_first() {
        let commit = Commit::new(
            "treehash".into(),
            None,
            "Xavier".into(),
            "Linux".into(),
            "ngdar-1.0.0".into(),
            1000,
            "Initial.".into(),
        );
        assert!(commit.parent_hash.is_none());
        let text = commit.to_text();
        assert!(text.contains("parent none"));
    }

    #[test]
    fn test_write_read_object() {
        let dir = tempfile::TempDir::new().unwrap();
        let objects_dir = dir.path().join("objects");
        let content =
            "type meta\nsize 100\nmtime 1000\npermissions 644\nbinary_hash abc\nvolume_id DVD\n";
        let hash = write_object(&objects_dir, content).unwrap();
        assert_eq!(hash.len(), 64);
        let read = read_object(&objects_dir, &hash).unwrap();
        assert_eq!(read, content);
    }
}
