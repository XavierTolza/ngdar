/// Integration tests for IgnoreRules and untracked file detection.
use ngdar::ignore::{list_untracked, IgnoreRules};

#[test]
fn test_ignore_dot_ngdar() {
    let rules = IgnoreRules::new(vec![]);
    assert!(rules.is_ignored(".ngdar/HEAD"));
    assert!(rules.is_ignored(".ngdarignore"));
}

#[test]
fn test_ignore_patterns() {
    let rules = IgnoreRules::new(vec!["*.log".into(), "tmp/".into()]);
    assert!(rules.is_ignored("debug.log"));
    assert!(rules.is_ignored("sub/debug.log"));
    assert!(rules.is_ignored("tmp/foo.txt"));
    assert!(rules.is_ignored("a/b/tmp/foo.txt"));
    assert!(!rules.is_ignored("src/main.rs"));
}

#[test]
fn test_untracked() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("tracked.txt"), "").unwrap();
    std::fs::write(dir.path().join("untracked.txt"), "").unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/other.txt"), "").unwrap();
    let tracked = vec!["tracked.txt".into()];
    let ut = list_untracked(dir.path(), &tracked).unwrap();
    assert!(ut.contains(&"untracked.txt".to_string()));
    assert!(ut.contains(&"sub/other.txt".to_string()));
    assert!(!ut.contains(&"tracked.txt".to_string()));
}
