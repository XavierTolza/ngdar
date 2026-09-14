/// Tests for `path_to_slash` — repository paths must always use `/`
/// regardless of the host platform, so archives and index entries stay
/// portable between Windows and Linux/macOS.
use std::path::Path;

#[test]
fn path_to_slash_uses_forward_slashes() {
    // Joining with the *native* separator proves the helper normalizes it:
    // on Windows this builds `docs\sub\file.txt`, on Unix `docs/sub/file.txt`.
    let native = Path::new("docs").join("sub").join("file.txt");
    assert_eq!(ngdar::path_to_slash(&native), "docs/sub/file.txt");
}

#[test]
fn path_to_slash_has_no_backslashes() {
    let native = Path::new("a").join("b").join("c");
    let slashed = ngdar::path_to_slash(&native);
    assert!(
        !slashed.contains('\\'),
        "repository paths must not contain backslashes: {slashed}"
    );
}
