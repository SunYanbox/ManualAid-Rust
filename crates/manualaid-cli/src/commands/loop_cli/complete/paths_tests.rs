use super::*;

use std::time::{Duration, Instant};

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

#[test]
fn unscanned_empty_query_stays_stable_across_calls() {
    // A default instance has no cached root view, so the empty query
    // exercises the cache-miss path with an empty directory key; the
    // second call must return the same entries from the populated cache.
    // 默认实例没有缓存的根视图，空查询因此走空目录键的缓存未命中路径；
    // 第二次调用必须从已填充的缓存返回相同条目。
    let mut candidates = PathCandidates::default();
    let first = candidates.filter("");
    let second = candidates.filter("");
    assert_eq!(first, second);
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

/// Push a cache past the refresh interval without waiting for wall-clock
/// time, so the stale path stays deterministic in tests.
/// 把缓存推到刷新间隔之外而无需真实等待墙钟时间，使过期路径在测试中
/// 保持确定性。
fn age_cache(candidates: &mut PathCandidates) {
    candidates.last_scan = Some(Instant::now() - REFRESH_INTERVAL - Duration::from_secs(1));
}

#[test]
fn stale_cache_is_rebuilt_on_the_next_query() {
    let root = temp_dir("complete-paths-stale");
    std::fs::write(root.join("first.txt"), "x").unwrap();
    let mut candidates = scanned(&root);
    assert_eq!(candidates.filter("first").len(), 1);

    // Change the tree behind the cache's back, then age it: the next query
    // must show the new file and drop the deleted one.
    // 在缓存不知情的情况下改动目录树，然后将其置为过期：下一次查询必须
    // 显示新文件并丢弃已删除的文件。
    std::fs::remove_file(root.join("first.txt")).unwrap();
    std::fs::write(root.join("second.txt"), "x").unwrap();
    age_cache(&mut candidates);

    assert!(candidates.filter("first").is_empty());
    assert_eq!(candidates.filter("second").len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fresh_cache_is_not_rebuilt() {
    let root = temp_dir("complete-paths-fresh");
    std::fs::write(root.join("first.txt"), "x").unwrap();
    let mut candidates = scanned(&root);

    // Inside the interval the cache is authoritative, so a deleted file
    // stays visible until the view ages out.
    // 在间隔内缓存即为准据，因此已删除的文件在视图过期前仍然可见。
    std::fs::remove_file(root.join("first.txt")).unwrap();
    assert_eq!(candidates.filter("first").len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn stale_cache_rebuilds_browsed_directory_contents() {
    let root = temp_dir("complete-paths-browsed-stale");
    let sub = root.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("main.rs"), "x").unwrap();
    let mut candidates = scanned(&root);
    assert_eq!(candidates.filter("sub/").len(), 1);

    // Browsed directory caches are populated on demand, so they must be
    // rebuilt along with the visible walk.
    // 浏览过的目录缓存是按需填充的，因此必须与可见遍历一同重建。
    std::fs::remove_file(sub.join("main.rs")).unwrap();
    age_cache(&mut candidates);
    assert!(candidates.filter("sub/").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn invalidated_cache_is_rebuilt_on_the_next_query() {
    let root = temp_dir("complete-paths-invalidated");
    std::fs::write(root.join("first.txt"), "x").unwrap();
    let mut candidates = scanned(&root);
    assert_eq!(candidates.filter("first").len(), 1);

    // A reference the cache offered turns out to be gone, so the loop
    // invalidates it; the rebuild must follow on the next query, without
    // waiting for the interval.
    // 缓存提供的引用被证实已不存在，loop 因此将其置为过期；重建必须在
    // 下一次查询发生，而不必等待该间隔。
    std::fs::remove_file(root.join("first.txt")).unwrap();
    candidates.invalidate();
    assert!(candidates.filter("first").is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
