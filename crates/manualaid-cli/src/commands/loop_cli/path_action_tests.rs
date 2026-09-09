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
async fn file_and_folder_references_run_as_path_user_actions() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("path-action-ok");
    std::fs::write(root.join("a.txt"), "file_body").unwrap();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub").join("inside.txt"), "inside").unwrap();

    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_path_actions(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "@a.txt @sub",
    )
    .await;

    assert!(handled);
    assert_eq!(session.len(), 1);
    let record = session.latest(1).unwrap();
    assert_eq!(record.results.len(), 2);
    assert_eq!(record.calls[0].tool_name, "read");
    assert!(record.results[0].success);
    assert!(record.results[0].output.contains("file_body"));
    assert_eq!(record.calls[1].tool_name, "path");
    assert!(record.results[1].success);
    assert!(record.results[1].output.contains("sub"));

    let copied = provider.read().unwrap();
    assert!(copied.contains("[USER_ACTION kind=\"path\" path="));
    assert!(copied.contains("[END USER_ACTION]"));
    assert!(!copied.contains("<system-reminder>"));
    assert!(copied.contains("file_body"));
    assert!(copied.contains("sub"));

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn lone_at_token_is_ignored() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("path-action-lone-at");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_path_actions(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "@",
    )
    .await;

    assert!(!handled);
    assert!(session.is_empty());
    assert!(provider.read().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn missing_path_prints_hint_without_recording_a_round() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("path-action-missing");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_path_actions(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "@missing",
    )
    .await;

    assert!(handled);
    assert!(session.is_empty());
    assert!(_capture.text().contains("does not exist"));
    assert!(provider.read().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&root);
}
