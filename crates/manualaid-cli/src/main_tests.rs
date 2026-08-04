//! Unit tests for the private CLI wiring in `main.rs`.
//! `main.rs` 中私有 CLI 装配代码的单元测试。

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use clap::Parser;

use super::{
    Cli, Command, current_dir, default_message, run, run_main, run_mask_with_home, run_restore,
    run_skill_with_home,
};

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("args should parse")
}

#[test]
fn parses_no_args() {
    let cli = parse(&["manualaid-cli"]);
    assert!(cli.command.is_none());
    assert_eq!(cli.lang, "en");
}

#[test]
fn parses_lang_flag() {
    let cli = parse(&["manualaid-cli", "--lang", "zh-CN"]);
    assert_eq!(cli.lang, "zh-CN");
}

#[test]
fn parses_mask_command() {
    let cli = parse(&["manualaid-cli", "mask", "hello"]);
    assert!(matches!(cli.command, Some(Command::Mask { input }) if input == "hello"));
}

#[test]
fn parses_restore_command() {
    let cli = parse(&["manualaid-cli", "restore", "text", "--snapshot", "s.json"]);
    assert!(matches!(
        cli.command,
        Some(Command::Restore { input, snapshot })
            if input == "text" && snapshot == PathBuf::from("s.json")
    ));
}

#[test]
fn parses_skill_flags() {
    let cli = parse(&["manualaid-cli", "skill", "--global", "--project"]);
    assert!(matches!(
        cli.command,
        Some(Command::Skill {
            global: true,
            project: true
        })
    ));
}

#[test]
fn default_message_is_localized() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    assert_eq!(default_message(), "ManualAid running...");
    i18n::set_locale("zh-CN");
    assert_eq!(default_message(), "ManualAid正在运行...");
}

#[test]
fn current_dir_resolves() {
    assert!(current_dir().is_ok());
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
    let dir = temp_dir("run-main-err");
    let snapshot = dir.join("snapshot.json");
    fs::write(&snapshot, "not json").unwrap();
    let cli = parse(&[
        "manualaid-cli",
        "restore",
        "[PRV_EMAIL_1]",
        "--snapshot",
        snapshot.to_str().unwrap(),
    ]);
    assert_eq!(run_main(cli), 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_restore_command_dispatches() {
    let dir = temp_dir("run-restore");
    let masked = dir.join("masked.txt");
    let snapshot = dir.join("snapshot.json");
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
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_roundtrip_returns_ok() {
    let dir = temp_dir("restore-ok");
    let masked = dir.join("masked.txt");
    let snapshot = dir.join("snapshot.json");
    fs::write(&masked, "contact [PRV_EMAIL_1]").unwrap();
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    assert!(run_restore(masked.to_str().unwrap(), &snapshot).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_empty_input_returns_ok() {
    let dir = temp_dir("restore-empty");
    let snapshot = dir.join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    assert!(run_restore("", &snapshot).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_invalid_snapshot_returns_localized_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = temp_dir("restore-err");
    let snapshot = dir.join("snapshot.json");
    fs::write(&snapshot, "not json").unwrap();
    let err = run_restore("[PRV_EMAIL_1]", &snapshot).unwrap_err();
    assert!(err.contains("Failed to parse snapshot"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_directory_input_uses_input_read_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = temp_dir("restore-invalid-path");
    let snapshot = dir.join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    let err = run_restore(dir.to_str().unwrap(), &snapshot).unwrap_err();
    assert!(err.contains("Failed to read input"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_missing_snapshot_returns_localized_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = temp_dir("restore-missing-snap");
    let err = run_restore("x", &dir.join("missing.json")).unwrap_err();
    assert!(err.contains("Failed to read snapshot"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_mask_with_explicit_home_returns_ok() {
    let dir = temp_dir("run-mask-home");
    assert!(run_mask_with_home("hello", &dir).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_mask_command_dispatches_with_explicit_home() {
    let dir = temp_dir("run-mask-dispatch");
    let cli = parse(&["manualaid-cli", "mask", "hello"]);
    assert!(run(cli, Some(&dir)).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_skill_with_explicit_home_returns_ok() {
    let dir = temp_dir("run-skill-home");
    assert!(run_skill_with_home(false, false, &dir).is_ok());
    assert!(run_skill_with_home(true, false, &dir).is_ok());
    assert!(run_skill_with_home(false, true, &dir).is_ok());
    assert!(run_skill_with_home(true, true, &dir).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn run_skill_command_dispatches_with_explicit_home() {
    let dir = temp_dir("run-skill-dispatch");
    let cli = parse(&["manualaid-cli", "skill", "--global", "--project"]);
    assert!(run(cli, Some(&dir)).is_ok());
    let _ = fs::remove_dir_all(&dir);
}

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "manualaid-cli-main-test-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
