use super::*;

/// Create a TAR archive for a commit (or a range of commits) already recorded
/// in the repository.
///
/// The archive always embeds the full `.ngdar/` metadata directory, so any
/// single archive describes the whole history. The binary data included is
/// limited to the files associated with the requested target:
///
/// - a single commit (given as a full hash, a unique prefix, or a volume ID),
///   or
/// - a range `<from>..<to>`, in which case only the files added or changed
///   between the two commits are packed.
///
/// When `verbose` is set, each file is listed as it is added and a progress
/// bar is shown (on a terminal only) while the archive is written.
pub fn pack(target: &str, out: &str, verbose: bool) -> Result<(), NgdarError> {
    let cwd = std::env::current_dir()?;
    let repo = Repository::find(&cwd)?;

    let (from, to) = parse_target(&repo, target)?;

    let meta_refs = resolve_pack_meta_refs(&repo, &from, &to)?;
    if meta_refs.is_empty() {
        eprintln!(
            "Warning: no files to pack for '{}' (they may all be missing on disk)",
            target
        );
    }

    // Total size of the files about to be written, for the progress bar.
    let total_size: u64 = meta_refs
        .iter()
        .filter_map(|(meta_hash, _)| {
            let text = objects::read_object(&repo.objects_path, meta_hash).ok()?;
            let meta = objects::Meta::from_text(&text).ok()?;
            std::fs::metadata(repo.path.join(&meta.path))
                .ok()
                .map(|m| m.len())
        })
        .sum();

    println!("Packing commit {}...", to);
    if from != to {
        println!("Range: {}..{}", from, to);
    }
    println!("Total size: {}", format_size(total_size));

    let (written, _warnings) = build_tar_archive(&repo, &meta_refs, out, total_size, verbose)?;

    println!("Created archive: {}", out);
    println!("Packed {} file(s).", written);
    Ok(())
}

/// Parse a `pack` target into a `(from, to)` commit-hash pair.
///
/// Accepts `<id>`, `<id>..<id>`, or `<id>...<id>`. Each `<id>` is a commit
/// hash (full or prefix) or a volume ID. A single id yields `(id, id)`.
fn parse_target(repo: &Repository, target: &str) -> Result<(String, String), NgdarError> {
    let (left, right) = if let Some((a, b)) = target.split_once("...") {
        (a, Some(b))
    } else if let Some((a, b)) = target.split_once("..") {
        (a, Some(b))
    } else {
        (target, None)
    };

    let from_id = resolve_id(repo, left)?;
    match right {
        Some(r) if !r.is_empty() => {
            let to_id = resolve_id(repo, r)?;
            Ok((from_id, to_id))
        }
        _ => Ok((from_id.clone(), from_id)),
    }
}

/// Resolve a single identifier: try as a commit id first, then as a volume ID.
fn resolve_id(repo: &Repository, id: &str) -> Result<String, NgdarError> {
    if let Ok(hash) = resolve_commit_id(repo, id) {
        return Ok(hash);
    }
    resolve_volume_id(repo, id)
}
