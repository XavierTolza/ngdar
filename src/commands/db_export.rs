use super::*;
use crate::objects::{self, Commit, Meta};

/// Export the object database as a CSV file.
pub fn db_export(csv_path: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    db_export_at(&cwd, csv_path)
}

fn db_export_at(cwd: &Path, csv_path: &str) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;

    let mut wtr = csv::Writer::from_path(csv_path)
        .map_err(|e| NgdarError::Other(format!("Cannot create CSV '{}': {}", csv_path, e)))?;

    // Write CSV header — like a `git log` export
    wtr.write_record([
        "commit_date",
        "commit_hash",
        "commit_msg",
        "filepath",
        "file_hash",
        "file_size",
        "volume_id",
    ])
    .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;

    let head = repo.read_head()?;
    let mut current = head;

    if current.is_none() {
        println!("(no commits yet)");
        wtr.flush()
            .map_err(|e| NgdarError::Other(format!("CSV flush error: {}", e)))?;
        return Ok(());
    }

    let mut count = 0u64;

    while let Some(hash) = current {
        let commit_text = objects::read_object(&repo.objects_path, &hash)?;
        let commit = Commit::from_text(&commit_text)?;

        let commit_date = chrono::DateTime::from_timestamp(commit.timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| commit.timestamp.to_string());

        let commit_msg = commit.message.replace('\n', " ");

        // Walk the tree to find all files in this commit
        let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
        let tree = objects::Tree::from_text(&tree_text)?;
        let mut meta_refs: Vec<(String, String)> = Vec::new();
        collect_meta_with_hashes(&repo.objects_path, &tree, &mut meta_refs, "")?;

        for (meta_hash, filepath) in &meta_refs {
            let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
            if let Ok(meta) = Meta::from_text(&meta_text) {
                let file_size = meta.size.to_string();
                wtr.write_record([
                    &commit_date,
                    &hash,
                    &commit_msg,
                    filepath,
                    &meta.binary_hash,
                    &file_size,
                    &meta.volume_id,
                ])
                .map_err(|e| NgdarError::Other(format!("CSV write error: {}", e)))?;
                count += 1;
            }
        }

        current = commit.parent_hash;
    }

    wtr.flush()
        .map_err(|e| NgdarError::Other(format!("CSV flush error: {}", e)))?;

    println!("Exported {} file-entry record(s) to {}", count, csv_path);
    Ok(())
}
