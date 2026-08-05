//! Integration tests for the public command dispatch and exit code logic.
//! 公共命令分发与退出码逻辑的集成测试。

use std::fs;
use std::sync::Mutex;

use clap::Parser;

use manualaid_cli::cli::Cli;
use manualaid_cli::commands::{run, run_main};

mod common;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("args should parse")
}

#[test]
fn run_no_args_prints_default_message() {
    let cli = parse(&["manualaid-cli"]);
    assert!(run(cli, None).is_ok());
}

#[test]
fn run_main_returns_success_for_no_args() {
    let cli = parse(&["manualaid-cli"]);
    assert_eq!(run_main(cli), 0);
}

#[test]
fn run_main_returns_failure_for_invalid_restore() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("run-main-err");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&snapshot, "not json").unwrap();
    let cli = parse(&[
        "manualaid-cli",
        "restore",
        "[PRV_EMAIL_1]",
        "--snapshot",
        snapshot.to_str().unwrap(),
    ]);
    assert_eq!(run_main(cli), 1);
}

#[test]
fn run_restore_command_dispatches() {
    let dir = common::TempDir::new("run-restore");
    let masked = dir.path().join("masked.txt");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&masked, "contact [PRV_EMAIL_1]").unwrap();
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    let masked_arg = masked.to_str().unwrap().to_string();
    let snapshot_arg = snapshot.to_str().unwrap().to_string();
    let cli = parse(&[
        "manualaid-cli",
        "restore",
        &masked_arg,
        "--snapshot",
        &snapshot_arg,
    ]);
    assert!(run(cli, None).is_ok());
}

#[test]
fn run_mask_command_dispatches_with_explicit_home() {
    let dir = common::TempDir::new("run-mask-dispatch");
    let cli = parse(&["manualaid-cli", "mask", "hello"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_skill_command_dispatches_with_explicit_home() {
    let dir = common::TempDir::new("run-skill-dispatch");
    let cli = parse(&["manualaid-cli", "skill", "--global", "--project"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_init_global_creates_home_dir() {
    let dir = common::TempDir::new("run-init-global");
    let cli = parse(&["manualaid-cli", "init", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(dir.path().join(".ManualAid").join("config.toml").is_file());
}

#[test]
fn run_dir_init_matches_init() {
    let dir = common::TempDir::new("run-dir-init");
    let cli = parse(&["manualaid-cli", "dir", "--init", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(dir.path().join(".ManualAid").join("config.toml").is_file());
}

#[test]
fn run_dir_view_missing_dir_is_ok() {
    let dir = common::TempDir::new("run-dir-view-missing");
    let cli = parse(&["manualaid-cli", "dir", "--view", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_dir_view_project_scope_with_explicit_values_is_ok() {
    let dir = common::TempDir::new("run-dir-view-project");
    let cli = parse(&[
        "manualaid-cli",
        "dir",
        "--view",
        "--project",
        "--depth",
        "0",
        "--limit",
        "5",
    ]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_dir_view_errors_when_manualaid_is_a_file() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("run-dir-view-file");
    fs::write(dir.path().join(".ManualAid"), "file").unwrap();
    let cli = parse(&["manualaid-cli", "dir", "--view", "--global"]);
    let err = run(cli, Some(dir.path())).unwrap_err();
    assert!(err.contains("Directory view failed"));
}

#[test]
fn run_dir_clean_with_yes_removes_dir() {
    let dir = common::TempDir::new("run-dir-clean");
    fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    fs::write(dir.path().join(".ManualAid").join("config.toml"), "[skill]").unwrap();
    let cli = parse(&["manualaid-cli", "dir", "--clean", "--global", "--yes"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(!dir.path().join(".ManualAid").exists());
}

#[test]
fn run_dir_clean_with_yes_when_missing_is_ok() {
    let dir = common::TempDir::new("run-dir-clean-missing");
    let cli = parse(&["manualaid-cli", "dir", "--clean", "--global", "--yes"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_dir_clean_without_yes_is_rejected_when_non_terminal() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("run-dir-clean-reject");
    fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    let cli = parse(&["manualaid-cli", "dir", "--clean", "--global"]);
    let err = run(cli, Some(dir.path())).unwrap_err();
    assert!(err.contains("Refusing to clean"));
    assert!(dir.path().join(".ManualAid").exists());
}
