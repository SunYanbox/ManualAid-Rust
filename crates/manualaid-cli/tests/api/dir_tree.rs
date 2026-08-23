//! API tests: directory tree rendering.
//! API 测试：目录树渲染。

use std::fs;

use manualaid_cli::dir_tree::{
    DEFAULT_VIEW_DEPTH, DEFAULT_VIEW_LIMIT, DirViewConfig, format_dir_tree,
};
use manualaid_core::error::CoreError;

use super::common;

fn view_config(depth: Option<usize>, limit: Option<usize>) -> DirViewConfig {
    DirViewConfig {
        depth,
        per_level_limit: limit,
    }
}

#[test]
fn dir_view_config_defaults() {
    let config = DirViewConfig::default();
    assert_eq!(config.depth, Some(DEFAULT_VIEW_DEPTH));
    assert_eq!(config.per_level_limit, Some(DEFAULT_VIEW_LIMIT));
}

#[test]
fn format_dir_tree_missing_root_is_not_found() {
    let tmp = common::TempDir::new("tree-missing");
    let err = format_dir_tree(&tmp.path().join(".ManualAid"), &DirViewConfig::default())
        .expect_err("tree should fail");
    assert!(matches!(err, CoreError::NotFound(_)));
}

#[test]
fn format_dir_tree_file_root_is_invalid_path() {
    let tmp = common::TempDir::new("tree-file");
    let path = tmp.path().join("blocker");
    fs::write(&path, "file").unwrap();
    let err = format_dir_tree(&path, &DirViewConfig::default()).expect_err("tree should fail");
    assert!(matches!(err, CoreError::InvalidPath(_)));
}

#[test]
fn format_dir_tree_empty_dir_prints_only_the_root() {
    let tmp = common::TempDir::new("tree-empty");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    let out = format_dir_tree(&root, &view_config(None, None)).unwrap();
    assert_eq!(out, format!("- {}", root.display()));
}

#[test]
fn format_dir_tree_sorts_dirs_first() {
    let tmp = common::TempDir::new("tree-sort");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("bb")).unwrap();
    fs::create_dir_all(root.join("aa")).unwrap();
    fs::write(root.join("zz.txt"), "").unwrap();
    fs::write(root.join("aa.txt"), "").unwrap();
    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], format!("- {}", root.display()));
    assert!(lines[1].ends_with("aa/"));
    assert!(lines[2].ends_with("bb/"));
    assert!(lines[3].ends_with("aa.txt"));
    assert!(lines[4].ends_with("zz.txt"));
}

#[test]
fn format_dir_tree_limits_files_per_level() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-file-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..10 {
        fs::write(root.join(format!("f{i}.txt")), "").unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), Some(7))).unwrap();
    assert!(out.contains("f6.txt"));
    assert!(!out.contains("f7.txt"));
    assert!(out.contains("… 3 more files"));
}

#[test]
fn format_dir_tree_limits_dirs_per_level() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-dir-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..8 {
        fs::create_dir_all(root.join(format!("d{i}"))).unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), Some(2))).unwrap();
    assert!(out.contains("d5/"));
    assert!(!out.contains("d6/"));
    assert!(out.contains("… 2 more dirs"));
}

#[test]
fn format_dir_tree_unlimited_limit_shows_everything() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-no-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..12 {
        fs::write(root.join(format!("f{i}.txt")), "").unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    assert!(out.contains("f11.txt"));
    assert!(!out.contains("more"));
}

#[test]
fn format_dir_tree_depth_controls_recursion() {
    let tmp = common::TempDir::new("tree-depth");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("a").join("b")).unwrap();
    fs::write(root.join("a").join("b").join("c.txt"), "").unwrap();

    let out = format_dir_tree(&root, &view_config(Some(0), None)).unwrap();
    assert_eq!(out, format!("- {}", root.display()));

    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    assert!(out.contains("a/"));
    assert!(!out.contains("b/"));

    let out = format_dir_tree(&root, &view_config(Some(2), None)).unwrap();
    assert!(out.contains("b/"));
    assert!(!out.contains("c.txt"));

    let out = format_dir_tree(&root, &view_config(None, None)).unwrap();
    assert!(out.contains("c.txt"));
}

#[test]
fn format_dir_tree_global_dir_budget_is_depth_first() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-global-budget");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    fs::create_dir_all(root.join(format!("d{i}/d{j}/d{k}/d{l}"))).unwrap();
                }
            }
        }
    }
    // limit 1: per-level dir cap 3, whole-tree cap 64; the tree has
    // 3 + 9 + 27 + 81 = 120 non-root dirs.
    let out = format_dir_tree(&root, &view_config(None, Some(1))).unwrap();
    let shown = out
        .lines()
        .filter(|line| line.contains("└── ") || line.contains("├── "))
        .filter(|line| line.ends_with('/'))
        .count();
    assert_eq!(shown, 64);
    assert!(out.contains("… 3 more dirs"));
    assert!(out.contains("… 2 more dirs"));
}

#[test]
fn format_dir_tree_keeps_connector_continuation_for_children() {
    let tmp = common::TempDir::new("tree-connectors");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(root.join("a").join("x.txt"), "").unwrap();
    fs::write(root.join("a").join("y.txt"), "").unwrap();
    fs::write(root.join("z.txt"), "").unwrap();
    let out = format_dir_tree(&root, &view_config(Some(2), None)).unwrap();
    assert!(out.contains("├── a/\n│   ├── x.txt\n│   └── y.txt"));
}
