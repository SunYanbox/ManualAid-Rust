//! Integration tests for the public command dispatch and exit code logic.
//! 公共命令分发与退出码逻辑的集成测试。

use std::fs;
use std::sync::Mutex;

use clap::Parser;

use manualaid_cli::cli::Cli;
use manualaid_cli::commands::{run, run_dir_clean_with_stdin, run_main};

mod common;
#[path = "commands/loop.rs"]
mod r#loop;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("args should parse")
}

/// Spawn the real binary with empty stdin, so the no-args interactive loop
/// can never block the test harness on a TTY.
/// 用空 stdin 启动真实二进制，避免无参数交互式 loop 在 TTY 上阻塞测试。
fn run_binary_with_empty_stdin() -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_manualaid-cli"))
        .stdin(std::process::Stdio::null())
        .output()
        .expect("spawn manualaid-cli binary")
}

#[test]
fn run_no_args_prints_default_message() {
    let output = run_binary_with_empty_stdin();
    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("ManualAid"),
        "stdout should contain the default startup message"
    );
}

#[test]
fn run_main_returns_success_for_no_args() {
    let output = run_binary_with_empty_stdin();
    assert!(output.status.success());
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
    let _capture = manualaid_cli::console::capture();
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
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-mask-dispatch");
    let cli = parse(&["manualaid-cli", "mask", "hello"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_skill_command_dispatches_with_explicit_home() {
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-skill-dispatch");
    let cli = parse(&["manualaid-cli", "skill", "--global", "--project"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_init_global_creates_home_dir() {
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-init-global");
    let cli = parse(&["manualaid-cli", "init", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(dir.path().join(".ManualAid").join("config.toml").is_file());
}

#[test]
fn run_dir_init_matches_init() {
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-dir-init");
    let cli = parse(&["manualaid-cli", "dir", "--init", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(dir.path().join(".ManualAid").join("config.toml").is_file());
}

#[test]
fn run_dir_view_missing_dir_is_ok() {
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-dir-view-missing");
    let cli = parse(&["manualaid-cli", "dir", "--view", "--global"]);
    assert!(run(cli, Some(dir.path())).is_ok());
}

#[test]
fn run_dir_view_project_scope_with_explicit_values_is_ok() {
    let _capture = manualaid_cli::console::capture();
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
    let _capture = manualaid_cli::console::capture();
    let dir = common::TempDir::new("run-dir-clean");
    fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    fs::write(dir.path().join(".ManualAid").join("config.toml"), "[skill]").unwrap();
    let cli = parse(&["manualaid-cli", "dir", "--clean", "--global", "--yes"]);
    assert!(run(cli, Some(dir.path())).is_ok());
    assert!(!dir.path().join(".ManualAid").exists());
}

#[test]
fn run_dir_clean_with_yes_when_missing_is_ok() {
    let _capture = manualaid_cli::console::capture();
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
    // Inject a non-terminal stdin so the test never blocks on a real
    // terminal prompt, regardless of how the test runner is started.
    let mut stdin = std::io::BufReader::new(std::io::empty());
    let err =
        run_dir_clean_with_stdin(false, true, false, dir.path(), false, &mut stdin).unwrap_err();
    assert!(err.contains("Refusing to clean"));
    assert!(dir.path().join(".ManualAid").exists());
}
