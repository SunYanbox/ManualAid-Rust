use super::*;

use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::clipboard::MockClipboard;
use manualaid_core::executor::Executor;
use manualaid_ws::session::SessionLog;
use std::sync::Arc;

fn build_test_executor(root: &std::path::Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(SessionMode::Manual),
        Arc::new(None),
    )
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn bang_runs_shell_command_and_records_round() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("bang-ok");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    run_bang_command(
        &provider,
        &executor,
        &mut config,
        &mut session,
        &mut options,
        "!echo hello",
    )
    .await;

    assert_eq!(session.len(), 1);
    let record = session.latest(1).unwrap();
    assert_eq!(record.calls.len(), 1);
    assert_eq!(record.calls[0].tool_name, "shell");
    assert_eq!(record.results.len(), 1);
    assert!(record.results[0].success);
    assert!(record.results[0].output.contains("hello"));
    assert!(record.results[0].audit_decisions.is_empty());
    assert!(provider.read().unwrap().contains("hello"));

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn bang_empty_command_prints_message_and_does_not_record() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("bang-empty");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    run_bang_command(
        &provider,
        &executor,
        &mut config,
        &mut session,
        &mut options,
        "!   ",
    )
    .await;

    assert!(session.is_empty());
    assert!(_capture.text().contains("Command must not be empty"));

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn bang_nonzero_exit_records_failure() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("bang-fail");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    #[cfg(windows)]
    let command = "!cmd /c exit 7";
    #[cfg(not(windows))]
    let command = "!sh -c 'exit 7'";

    run_bang_command(
        &provider,
        &executor,
        &mut config,
        &mut session,
        &mut options,
        command,
    )
    .await;

    assert_eq!(session.len(), 1);
    let record = session.latest(1).unwrap();
    assert!(!record.results[0].success);
    assert!(record.results[0].output.contains("exited with code 7"));

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn bang_blacklisted_command_is_denied_and_recorded() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("bang-denied");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    run_bang_command(
        &provider,
        &executor,
        &mut config,
        &mut session,
        &mut options,
        "!echo mkfs.",
    )
    .await;

    assert_eq!(session.len(), 1);
    let record = session.latest(1).unwrap();
    assert!(!record.results[0].success);
    assert!(record.results[0].output.contains("denied"));

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn bang_auto_copy_off_asks_copy_and_writes_on_yes() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("bang-copy");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        auto_copy: false,
        ..LoopOptions::default()
    };
    super::super::utils::push_test_input(&["y"]);

    run_bang_command(
        &provider,
        &executor,
        &mut config,
        &mut session,
        &mut options,
        "!echo copy_me",
    )
    .await;

    let output = _capture.text();
    assert!(output.contains("Copy this round's tool call results"));
    assert!(provider.read().unwrap().contains("copy_me"));

    let _ = std::fs::remove_dir_all(&root);
}
