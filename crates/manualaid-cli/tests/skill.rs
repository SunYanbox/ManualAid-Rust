//! Integration tests for the public `skill` command handler.
//! 公共 `skill` 命令处理函数的集成测试。

use manualaid_cli::commands::{run_skill, run_skill_with_home};

mod common;

#[test]
fn run_skill_with_explicit_home_returns_ok() {
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-skill-home");
    assert!(run_skill_with_home(false, false, dir.path()).is_ok());
    assert!(run_skill_with_home(true, false, dir.path()).is_ok());
    assert!(run_skill_with_home(false, true, dir.path()).is_ok());
    assert!(run_skill_with_home(true, true, dir.path()).is_ok());
}

#[test]
fn run_skill_without_home_falls_back_to_real_home() {
    let _capture = manualaid_cli::console::capture();
    assert!(run_skill(false, true, None).is_ok());
}
