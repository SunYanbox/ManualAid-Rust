use super::*;

use crate::test_support::temp_dir;

/// Join path parts with the platform separator so assertions match the
/// stored representation on every OS.
/// 用平台分隔符拼接路径段，让断言在各操作系统上都与存储形式一致。
fn rel(parts: &[&str]) -> String {
    parts.join(std::path::MAIN_SEPARATOR_STR)
}

#[test]
fn separator_only_query_lists_root_children() {
    let root = temp_dir("complete-paths-separator-only");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    let mut candidates = scanned(&root);
    let entries = candidates.filter("/");
    assert_eq!(labels(&entries), vec!["sub"]);
    let _ = std::fs::remove_dir_all(&root);
}

fn scanned(root: &std::path::Path) -> PathCandidates {
    let mut candidates = PathCandidates::default();
    candidates.scan(root);
    candidates
}

/// Return the labels of `entries`, for assertions that only care about
/// paths.
/// 返回 `entries` 的标签列表，供只关心路径的断言使用。
fn labels(entries: &[PathEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.label.as_str()).collect()
}

#[test]
fn empty_query_lists_the_top_level_entries() {
    let root = temp_dir("complete-paths-top-level");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("readme.md"), "x").unwrap();
    std::fs::write(root.join("sub").join("main.rs"), "x").unwrap();
    let mut candidates = scanned(&root);
    let out = candidates.filter("");
    let names = labels(&out);
    assert!(names.contains(&"sub"));
    assert!(names.contains(&"readme.md"));
    assert!(!names.contains(&"main.rs"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn trailing_separator_lists_directory_children() {
    let root = temp_dir("complete-paths-children");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub").join("main.rs"), "x").unwrap();
    std::fs::write(root.join("sub").join("other.rs"), "x").unwrap();
    let mut candidates = scanned(&root);
    let out = candidates.filter("sub/");
    let names = labels(&out);
    assert!(names.contains(&rel(&["sub", "main.rs"]).as_str()));
    assert!(names.contains(&rel(&["sub", "other.rs"]).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn multi_segment_query_lists_directory_children_only() {
    let root = temp_dir("complete-paths-visible");
    let page = root.join("src").join("page");
    std::fs::create_dir_all(&page).unwrap();
    std::fs::write(page.join("mod.rs"), "x").unwrap();
    let mut candidates = scanned(&root);
    let out = candidates.filter("src/pa");
    let names = labels(&out);
    assert!(names.contains(&rel(&["src", "page"]).as_str()));
    assert!(!names.contains(&rel(&["src", "page", "mod.rs"]).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn single_segment_search_matches_case_insensitive_basename() {
    let root = temp_dir("complete-paths-case");
    let page = root.join("src").join("page");
    std::fs::create_dir_all(&page).unwrap();
    std::fs::write(page.join("mod.rs"), "x").unwrap();
    let mut candidates = scanned(&root);
    assert!(labels(&candidates.filter("MOD")).contains(&rel(&["src", "page", "mod.rs"]).as_str()));
    assert!(candidates.filter("nomatch").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn filter_accepts_forward_and_backward_separators() {
    let root = temp_dir("complete-paths-sep");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub").join("item.txt"), "x").unwrap();
    let mut candidates = scanned(&root);
    assert!(labels(&candidates.filter("sub/item")).contains(&rel(&["sub", "item.txt"]).as_str()));
    assert!(labels(&candidates.filter("sub\\item")).contains(&rel(&["sub", "item.txt"]).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ignored_directory_is_dimmed_and_browsable() {
    let root = temp_dir("complete-paths-ignored");
    std::fs::write(root.join(".gitignore"), ".ManualAid/\n").unwrap();
    let manualaid = root.join(".ManualAid");
    std::fs::create_dir_all(&manualaid).unwrap();
    std::fs::write(manualaid.join(".gitignore"), "*\n").unwrap();
    std::fs::write(manualaid.join("keep.txt"), "x").unwrap();
    std::fs::write(manualaid.join("plans.txt"), "x").unwrap();
    let mut candidates = scanned(&root);
    let top = candidates.filter("");
    let entry = top
        .iter()
        .find(|entry| entry.label == ".ManualAid")
        .unwrap();
    assert!(entry.is_dir);
    assert!(entry.dimmed);
    let children = candidates.filter(".ManualAid/");
    assert!(labels(&children).contains(&rel(&[".ManualAid", "keep.txt"]).as_str()));
    assert!(labels(&children).contains(&rel(&[".ManualAid", "plans.txt"]).as_str()));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ignored_directory_is_reachable_by_name() {
    let root = temp_dir("complete-paths-ignored-reach");
    let manualaid = root.join(".ManualAid");
    std::fs::create_dir_all(&manualaid).unwrap();
    std::fs::write(manualaid.join(".gitignore"), "*\n").unwrap();
    let mut candidates = scanned(&root);
    let matches = candidates.filter(".manual");
    assert!(labels(&matches).contains(&".ManualAid"));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rescan_replaces_the_cached_entries() {
    let root = temp_dir("complete-paths-rescan");
    std::fs::write(root.join("first.txt"), "x").unwrap();
    let mut candidates = PathCandidates::default();
    candidates.scan(&root);
    assert_eq!(candidates.filter("first").len(), 1);
    std::fs::remove_file(root.join("first.txt")).unwrap();
    std::fs::write(root.join("second.txt"), "x").unwrap();
    candidates.scan(&root);
    assert!(candidates.filter("first").is_empty());
    assert_eq!(candidates.filter("second").len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}
