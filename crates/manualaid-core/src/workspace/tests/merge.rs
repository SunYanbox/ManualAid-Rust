use std::path::PathBuf;

use super::*;

#[test]
fn merge_exempt_paths_deduplicates_and_sorts() {
    let a = PathBuf::from("/b");
    let b = PathBuf::from("/a");
    let merged = merge_exempt_paths(&[a.clone(), b.clone()], &[b, PathBuf::from("/c")]);
    assert_eq!(
        merged,
        vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c")
        ]
    );
}

#[cfg(windows)]
#[test]
fn merge_exempt_paths_dedups_ignoring_ascii_case_on_windows() {
    let merged = merge_exempt_paths(
        &[PathBuf::from(r"C:\Foo\Bar")],
        &[PathBuf::from(r"c:\foo\bar")],
    );
    assert_eq!(merged.len(), 1);
}
