//! Tests for input, submit and paste handlers.
//! 输入、提交与粘贴处理器的测试。

use crate::LOCALE_LOCK;
use crate::common;
use manualaid_cli::commands::loop_cli::{
    LoopOptions, ask_copy, input_and_submit, paste_and_submit_with_provider, push_test_input,
    submit_text, submit_text_with_provider,
};
use manualaid_core::clipboard::{ClipboardProvider, MockClipboard};
use manualaid_core::parser::FormatRegistry;
use manualaid_ws::session::SessionLog;

#[test]
fn ask_copy_accepts_yes_ignores_rest() {
    let _capture = manualaid_cli::console::capture();
    push_test_input(&["y"]);
    assert!(ask_copy());
    push_test_input(&["Y"]);
    assert!(ask_copy());
    push_test_input(&["n"]);
    assert!(!ask_copy());
    push_test_input(&[""]);
    assert!(!ask_copy());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn input_and_submit_eof_without_text_is_noop() {
    let _capture = manualaid_cli::console::capture();
    let root = common::TempDir::new("input-eof");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    push_test_input(&[]);
    input_and_submit(
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn input_and_submit_end_marker_without_text_is_noop() {
    let _capture = manualaid_cli::console::capture();
    let root = common::TempDir::new("input-marker");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    push_test_input(&["/end"]);
    input_and_submit(
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn input_and_submit_executes_typed_round() {
    let _capture = manualaid_cli::console::capture();
    let root = common::TempDir::new("input-round");
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        auto_copy: false,
        ..LoopOptions::default()
    };
    push_test_input(&[&super::read_call(root.path()), "/end", "n"]);
    input_and_submit(
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 1);
    assert_eq!(session.total_calls(), 1);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn submit_text_parse_error_prints_message_and_keeps_session_empty() {
    let _capture = manualaid_cli::console::capture();
    let root = common::TempDir::new("submit-parse");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    submit_text(
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        "not a tool call",
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[tokio::test]
async fn submit_text_with_auto_copy_asks_and_skips_copy_on_no() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("submit-autocopy");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    push_test_input(&["n"]);
    submit_text_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &super::read_call(root.path()),
        100,
    )
    .await;
    assert_eq!(session.len(), 1);
}

#[tokio::test]
async fn submit_text_with_auto_copy_writes_to_clipboard() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("submit-autocopy-write");
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        auto_copy: true,
        ..LoopOptions::default()
    };
    push_test_input(&["y"]);
    submit_text_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &super::read_call(root.path()),
        100,
    )
    .await;
    assert_eq!(session.len(), 1);
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains("hello"));
}

#[tokio::test]
async fn submit_text_with_auto_copy_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    let root = common::TempDir::new("submit-autocopy-err");
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        auto_copy: true,
        ..LoopOptions::default()
    };
    submit_text_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &super::read_call(root.path()),
        100,
    )
    .await;
    assert_eq!(session.len(), 1);
}

#[tokio::test]
async fn paste_and_submit_pastes_clipboard_text_as_a_round() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("paste-round");
    mock.write(&super::read_call(root.path())).unwrap();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    push_test_input(&["n"]);
    paste_and_submit_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 1);
}

#[tokio::test]
async fn paste_and_submit_with_empty_clipboard_is_noop() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("paste-empty");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    paste_and_submit_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[tokio::test]
async fn paste_and_submit_with_read_error_is_noop() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_read_error("mock read failure");
    let root = common::TempDir::new("paste-err");
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    paste_and_submit_with_provider(
        &mock,
        &super::executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn submit_text_records_round_stats() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = common::TempDir::new("submit-stats");
    let registry = FormatRegistry::new();
    let exec = super::executor(root.path());
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        auto_copy: false,
        ..LoopOptions::default()
    };
    push_test_input(&["n"]);
    submit_text(
        &exec,
        &registry,
        &mut session,
        &mut options,
        &super::read_call(root.path()),
        50000,
    )
    .await;
    let stats = session.latest(1).expect("round recorded").stats;
    assert!(stats.total_tokens > 0);
}
