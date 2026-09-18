/// Integration tests for `HashProxy` — the cache-aware file hash accessor.
///
/// These exercise the real filesystem: the proxy must return the cached hash
/// while a file is unchanged, and recompute it as soon as the file changes
/// (including a same-size rewrite within the same second).
use ngdar::hasher::HashProxy;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

fn write(root: &Path, name: &str, content: &str) {
    std::fs::write(root.join(name), content).unwrap();
}

/// Set a file's mtime to a specific instant so a test can force a change
/// without relying on filesystem timestamp granularity.
fn set_mtime(root: &Path, name: &str, secs: u64, nanos: u32) {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(root.join(name))
        .unwrap();
    let t = UNIX_EPOCH + Duration::new(secs, nanos);
    file.set_modified(t).unwrap();
}

#[test]
fn hash_returns_known_blake3_value() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", "hello world");

    let mut proxy = HashProxy::load(root, &root.join("cache.tsv")).unwrap();
    assert_eq!(
        proxy.hash("a.txt").unwrap(),
        "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24"
    );
}

#[test]
fn unchanged_file_serves_hash_from_cache() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", "hello world");

    let mut proxy = HashProxy::load(root, &root.join("cache.tsv")).unwrap();
    let first = proxy.hash("a.txt").unwrap();

    // A second lookup with the same (size, mtime) must return the cached value.
    assert_eq!(proxy.hash("a.txt").unwrap(), first);
    assert_eq!(proxy.cached_hash("a.txt"), Some(first.as_str()));
}

#[test]
fn changed_content_is_rehashed() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", "hello world");

    let mut proxy = HashProxy::load(root, &root.join("cache.tsv")).unwrap();
    let first = proxy.hash("a.txt").unwrap();

    write(root, "a.txt", "hello world, extended");

    let second = proxy.hash("a.txt").unwrap();
    assert_ne!(first, second, "hash must change when content changes");
    assert_eq!(proxy.cached_hash("a.txt"), Some(second.as_str()));
}

/// A same-size rewrite within the same second is the classic stale-cache trap.
/// Both writes share the same whole-second mtime, so only sub-second resolution
/// reveals the change.
#[test]
fn same_size_rewrite_in_same_second_is_rehashed() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", "will change"); // 11 bytes
    set_mtime(root, "a.txt", 1_700_000_000, 100_000_000);

    let mut proxy = HashProxy::load(root, &root.join("cache.tsv")).unwrap();
    let first = proxy.hash("a.txt").unwrap();

    // Same size, same second, later nanosecond mtime.
    write(root, "a.txt", "changed now"); // also 11 bytes
    set_mtime(root, "a.txt", 1_700_000_000, 900_000_000);

    let second = proxy.hash("a.txt").unwrap();
    assert_ne!(
        first, second,
        "a same-size rewrite with a later sub-second mtime must be re-hashed"
    );
}

#[test]
fn cached_hash_is_none_for_missing_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    write(root, "a.txt", "hello world");

    let mut proxy = HashProxy::load(root, &root.join("cache.tsv")).unwrap();
    proxy.hash("a.txt").unwrap();

    std::fs::remove_file(root.join("a.txt")).unwrap();
    assert!(proxy.cached_hash("a.txt").is_none());
}

#[test]
fn cache_survives_save_and_reload() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    let cache = root.join("cache.tsv");
    write(root, "a.txt", "hello world");

    let expected = {
        let mut proxy = HashProxy::load(root, &cache).unwrap();
        let h = proxy.hash("a.txt").unwrap();
        proxy.save().unwrap();
        h
    };

    let proxy = HashProxy::load(root, &cache).unwrap();
    assert_eq!(proxy.cached_hash("a.txt"), Some(expected.as_str()));
}

/// Caches written by earlier versions stored whole-second mtimes. Those must
/// still be honoured (so upgrading does not flag every file as modified) and
/// get upgraded to the nanosecond fingerprint on the next refresh.
#[test]
fn legacy_second_resolution_cache_is_accepted_then_upgraded() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    let cache = root.join("cache.tsv");
    write(root, "a.txt", "hello world");

    let metadata = std::fs::metadata(root.join("a.txt")).unwrap();
    let secs = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let expected = "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24";
    std::fs::write(&cache, format!("11\t{secs}\t{expected}\ta.txt\n")).unwrap();

    let mut proxy = HashProxy::load(root, &cache).unwrap();
    // Legacy seconds entry: served as-is without re-hashing.
    assert_eq!(proxy.cached_hash("a.txt"), Some(expected));
    assert_eq!(proxy.hash("a.txt").unwrap(), expected);

    // After a refresh the fingerprint is stored at nanosecond resolution.
    proxy.save().unwrap();
    let stored = std::fs::read_to_string(&cache).unwrap();
    let mtime_field: i64 = stored.split('\t').nth(1).unwrap().trim().parse().unwrap();
    assert!(
        mtime_field > secs as i64,
        "cache should now store nanoseconds, got {mtime_field}"
    );
}
