//! Integration tests for the public `mask` command handler.
//! 公共 `mask` 命令处理函数的集成测试。

use manualaid_cli::commands::run_mask_with_home;

mod common;

#[test]
fn run_mask_with_explicit_home_returns_ok() {
    let dir = common::TempDir::new("run-mask-home");
    assert!(run_mask_with_home("hello", dir.path()).is_ok());
}
