use super::*;

/// Extract files from a specific commit into an output directory.
pub fn export(commit_hash: &str, out_path: &str) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;
    export_with_repo(&repo, commit_hash, out_path)
}

fn export_with_repo(
    repo: &Repository,
    commit_hash: &str,
    out_path: &str,
) -> Result<(), NgdarError> {
    let meta_refs = collect_commit_meta_refs(repo, commit_hash)?;
    println!(
        "Exporting {} file(s) from commit {}...",
        meta_refs.len(),
        commit_hash
    );
    let file = std::fs::File::create(out_path)?;
    let mut builder = tar::Builder::new(&file);
    add_dir_to_tar(&mut builder, &repo.ngdar_path, ".ngdar", &repo.ngdar_path)?;
    let (exported, warnings) = append_meta_files_to_tar(
        &mut builder,
        repo,
        &meta_refs,
        &ProgressBar::hidden(),
        false,
    )?;
    builder.finish()?;
    println!("Exported {} file(s) to {}", exported, out_path);
    if warnings > 0 {
        eprintln!("{} file(s) were skipped due to warnings", warnings);
    }
    Ok(())
}
