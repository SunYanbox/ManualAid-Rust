use std::path::{Path, PathBuf};

use super::*;

#[test]
fn normalize_makes_relative_paths_absolute() {
    let normalized = normalize_path(Path::new("a/b"));
    assert!(normalized.is_absolute());
    assert!(normalized.ends_with("a/b"));
}

#[test]
fn normalize_strips_dot_and_parent_components() {
    let base = std::env::current_dir().unwrap();
    let input = base.join("a/./b/../c");
    let normalized = normalize_path(&input);
    assert_eq!(normalized, base.join("a/c"));
}

#[test]
fn lexical_normalize_skips_dot_components_and_keeps_plain_paths() {
    assert_eq!(lexical_normalize(Path::new("a/./b")), PathBuf::from("a/b"));
    assert_eq!(
        strip_verbatim(PathBuf::from("plain/path")),
        PathBuf::from("plain/path")
    );
}

#[test]
fn normalize_caps_parent_above_root() {
    #[cfg(windows)]
    let root = Path::new("C:\\");
    #[cfg(not(windows))]
    let root = Path::new("/");
    let normalized = normalize_path(&root.join("..").join("x"));
    assert_eq!(normalized, root.join("x"));
}
