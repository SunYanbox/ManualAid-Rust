//! Integration tests for the public `restore` command handler.
//! 公共 `restore` 命令处理函数的集成测试。

use std::fs;
use std::sync::Mutex;

use manualaid_cli::commands::run_restore;

mod common;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn restore_roundtrip_returns_ok() {
    let dir = common::TempDir::new("restore-ok");
    let masked = dir.path().join("masked.txt");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&masked, "contact [PRV_EMAIL_1]").unwrap();
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    assert!(run_restore(masked.to_str().unwrap(), &snapshot).is_ok());
}

#[test]
fn restore_empty_input_returns_ok() {
    let dir = common::TempDir::new("restore-empty");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    assert!(run_restore("", &snapshot).is_ok());
}

#[test]
fn restore_invalid_snapshot_returns_localized_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("restore-err");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&snapshot, "not json").unwrap();
    let err = run_restore("[PRV_EMAIL_1]", &snapshot).unwrap_err();
    assert!(err.contains("Failed to parse snapshot"));
}

#[test]
fn restore_directory_input_uses_input_read_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("restore-invalid-path");
    let snapshot = dir.path().join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    let err = run_restore(dir.path().to_str().unwrap(), &snapshot).unwrap_err();
    assert!(err.contains("Failed to read input"));
}

#[test]
fn restore_missing_snapshot_returns_localized_error() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = common::TempDir::new("restore-missing-snap");
    let err = run_restore("x", &dir.path().join("missing.json")).unwrap_err();
    assert!(err.contains("Failed to read snapshot"));
}
