use super::*;
use crate::objects::{self, Commit, Meta};

pub fn export(commit_hash: &str, out_path: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    export_with_repo(&repo, commit_hash, out_path)
}

#[cfg(test)]
fn export_at(cwd: &Path, commit_hash: &str, out_path: &str) -> Result<(), NgdarError> {
    let repo = Repository::find(cwd)?;
    // Reuse the same logic as export but with given cwd
    let _ = repo; // we already have the repo, but the logic needs the path
    export_with_repo(&repo, commit_hash, out_path)
}

fn export_with_repo(
    repo: &Repository,
    commit_hash: &str,
    out_path: &str,
) -> Result<(), NgdarError> {
    let commit_text = objects::read_object(&repo.objects_path, commit_hash)?;
    let commit = Commit::from_text(&commit_text)?;
    let tree_text = objects::read_object(&repo.objects_path, &commit.tree_hash)?;
    let tree = objects::Tree::from_text(&tree_text)?;
    let mut meta_refs: Vec<(String, String)> = Vec::new();
    collect_meta_with_hashes(&repo.objects_path, &tree, &mut meta_refs, "")?;
    println!(
        "Exporting {} file(s) from commit {}...",
        meta_refs.len(),
        commit_hash
    );
    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;
    let mut exported = 0u64;
    let mut warnings = 0u64;
    for (meta_hash, _rel_path) in &meta_refs {
        let meta_text = objects::read_object(&repo.objects_path, meta_hash)?;
        if let Ok(meta) = Meta::from_text(&meta_text) {
            let full_path = repo.path.join(&meta.path);
            if !full_path.exists() {
                eprintln!("Warning: '{}' not found on disk, skipping", meta.path);
                warnings += 1;
                continue;
            }
            let actual_hash = hash_file(&full_path)?;
            let actual_hex = hash_to_hex(&actual_hash);
            if actual_hex != meta.binary_hash {
                eprintln!(
                    "Warning: '{}' has been modified (hash mismatch), skipping",
                    meta.path
                );
                warnings += 1;
                continue;
            }
            let file_data = std::fs::read(&full_path)?;
            let file_metadata = std::fs::metadata(&full_path)?;
            let mut header = tar::Header::new_gnu();
            header.set_size(file_data.len() as u64);
            header.set_mode(format_permissions(&file_metadata));
            header.set_mtime(get_mtime(&file_metadata) as u64);
            header.set_entry_type(tar::EntryType::Regular);
            builder
                .append_data(&mut header, &meta.path, std::io::Cursor::new(&file_data))
                .map_err(|e| NgdarError::Other(format!("TAR error: {}", e)))?;
            exported += 1;
        }
    }
    builder.finish()?;
    println!("Exported {} file(s) to {}", exported, out_path);
    if warnings > 0 {
        eprintln!("{} file(s) were skipped due to warnings", warnings);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Repository;

    #[test]
    fn test_export_nonexistent_commit() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        Repository::init(&root).unwrap();
        let result = export_at(
            &root,
            "0000000000000000000000000000000000000000000000000000000000000000",
            "out.tar",
        );
        assert!(result.is_err(), "export() should fail on bad commit hash");
    }
}
