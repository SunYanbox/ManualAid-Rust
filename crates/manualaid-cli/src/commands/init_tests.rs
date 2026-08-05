//! Unit tests for the crate-private init helper in `init.rs`.
//! `init.rs` 中 crate 私有初始化辅助函数的单元测试。

use super::init_manualaid_dirs;
use crate::test_support::temp_dir;

#[test]
fn init_project_scope_creates_only_project_dir() {
    let project = temp_dir("init-project");
    let home = temp_dir("init-home");
    let lines = init_manualaid_dirs(true, false, &project, &home).unwrap();
    assert_eq!(lines.len(), 1);
    assert!(project.join(".ManualAid").join("config.toml").is_file());
    assert!(!home.join(".ManualAid").exists());
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn init_both_scopes_creates_both_dirs() {
    let project = temp_dir("init-both-project");
    let home = temp_dir("init-both-home");
    let lines = init_manualaid_dirs(true, true, &project, &home).unwrap();
    assert_eq!(lines.len(), 2);
    assert!(project.join(".ManualAid").join("config.toml").is_file());
    assert!(home.join(".ManualAid").join("config.toml").is_file());
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&home);
}
