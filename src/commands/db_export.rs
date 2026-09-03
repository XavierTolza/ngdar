use super::*;
use crate::objects::Meta;

/// Export the object database as a CSV file.
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
