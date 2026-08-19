use std::path::Path;

use super::*;

#[test]
fn lock_name_is_stable_and_contains_no_separators() {
    let path = Path::new("/home/alice/.ManualAid/config.toml");
    let first = lock_name(path);
    assert_eq!(first, lock_name(path));
    assert!(!first.is_empty());
    assert!(!first.contains(['/', '\\']));
    assert_ne!(lock_name(Path::new("/home/alice/.ManualAid/a")), first);
}

#[test]
fn lock_name_is_case_and_separator_insensitive() {
    let windows = Path::new(r"C:\Users\Alice\.ManualAid\config.toml");
    let normalized = Path::new("c:/users/alice/.manualaid/config.toml");
    assert_eq!(lock_name(windows), lock_name(normalized));
}
