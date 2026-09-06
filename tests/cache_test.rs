/// Integration tests for `CacheStore` (loading, inserting, looking up, saving).
use ngdar::cache::CacheStore;

#[test]
fn test_cache_empty_load() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("cache.tsv");
    let cache = CacheStore::load(&path).unwrap();
    assert!(cache.entries.is_empty());
}

#[test]
fn test_cache_insert_and_lookup() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("cache.tsv");
    let mut cache = CacheStore::load(&path).unwrap();
    cache.insert(1024, 1700000000, "abc123".into(), "file.txt".into());
    assert_eq!(cache.lookup(1024, 1700000000, "file.txt"), Some("abc123"));
    assert!(cache.lookup(2048, 1700000000, "file.txt").is_none());
}

#[test]
fn test_cache_persistence() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("cache.tsv");
    {
        let mut cache = CacheStore::load(&path).unwrap();
        cache.insert(512, 1700000001, "def456".into(), "doc.pdf".into());
        cache.save().unwrap();
    }
    {
        let cache = CacheStore::load(&path).unwrap();
        assert_eq!(cache.lookup(512, 1700000001, "doc.pdf"), Some("def456"));
    }
}

#[test]
fn test_cache_update() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("cache.tsv");
    let mut cache = CacheStore::load(&path).unwrap();
    cache.insert(100, 1000, "old".into(), "f.txt".into());
    cache.insert(200, 2000, "new".into(), "f.txt".into());
    assert_eq!(cache.lookup(200, 2000, "f.txt"), Some("new"));
    assert!(cache.lookup(100, 1000, "f.txt").is_none());
}
