use crate::common;
use crate::{LOCALE_LOCK, STYLE_LOCK};
use manualaid_cli::commands::loop_cli::{
    LoopOptions, ask_copy, copy_intent_rule_with_provider, copy_round_result,
    copy_round_result_with_provider, copy_system_prompt_with_provider, input_and_submit,
    paste_and_submit_with_provider, print_session_summary, push_test_input, show_tool_history,
    submit_text, submit_text_with_provider, truncate_preview_lines,
};
use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::clipboard::{ClipboardProvider, MockClipboard};
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_ws::config::Config;
use manualaid_ws::session::{RoundStats, SessionLog};
use std::path::Path;
use std::sync::Arc;

fn executor(root: &Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(SessionMode::AcceptEdit),
        Arc::new(None),
    )
}

fn read_call(root: &Path) -> String {
    let file = root.join("target.txt");
    std::fs::write(&file, "hello").unwrap();
    format!("<read><file_path>{}</file_path></read>", file.display())
}

async fn session_with_round(root: &Path) -> SessionLog {
    let mut session = SessionLog::new();
    add_round(root, &mut session, RoundStats::default()).await;
    session
}

async fn add_round(root: &Path, session: &mut SessionLog, stats: RoundStats) {
    let registry = FormatRegistry::new();
    let calls = registry.parse(&read_call(root)).unwrap().calls;
    let exec = executor(root);
    let mut results = Vec::new();
    for call in &calls {
        results.push(exec.execute(call.clone()).await);
    }
    session.push(calls, results, stats);
}

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
// The lock must span the await so no concurrent test flips the
// process-wide locale while this test prints localized text.
// 锁须跨 await 持有，避免并发测试在本测试输出本地化文本时切换全局 locale。
#[allow(clippy::await_holding_lock)]
async fn print_session_summary_lists_stats() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = common::TempDir::new("summary");
    let session = session_with_round(root.path()).await;
    print_session_summary(&Config::default(), &session);
}

#[test]
fn copy_round_result_without_rounds_prints_notice() {
    let _capture = manualaid_cli::console::capture();
    let session = SessionLog::new();
    copy_round_result(&session, 100);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn copy_round_result_rejects_out_of_range_index() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = common::TempDir::new("copy-index");
    let session = session_with_round(root.path()).await;
    push_test_input(&["9"]);
    copy_round_result(&session, 100);
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
        &executor(root.path()),
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
        &executor(root.path()),
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
    push_test_input(&[&read_call(root.path()), "/end", "n"]);
    input_and_submit(
        &executor(root.path()),
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
        &executor(root.path()),
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
        &executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &read_call(root.path()),
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
        &executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &read_call(root.path()),
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
        &executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        &read_call(root.path()),
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
    mock.write(&read_call(root.path())).unwrap();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();
    push_test_input(&["n"]);
    paste_and_submit_with_provider(
        &mock,
        &executor(root.path()),
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
        &executor(root.path()),
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
        &executor(root.path()),
        &FormatRegistry::new(),
        &mut session,
        &mut options,
        100,
    )
    .await;
    assert_eq!(session.len(), 0);
}

#[test]
fn copy_system_prompt_writes_prompt_to_clipboard() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-prompt");
    copy_system_prompt_with_provider(
        &mock,
        &Config::default(),
        root.path(),
        &FormatRegistry::new(),
    );
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains("<read>"));
}

#[test]
fn copy_system_prompt_includes_selected_context_files() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-prompt-context");
    std::fs::write(root.path().join("AGENTS.md"), "# project rules").unwrap();
    copy_system_prompt_with_provider(
        &mock,
        &Config::default(),
        root.path(),
        &FormatRegistry::new(),
    );
    let clipboard = mock.read().unwrap();
    let dynamic = clipboard
        .split_once("<dynamic-context>")
        .and_then(|(_, rest)| rest.split_once("</dynamic-context>"))
        .map(|(inner, _)| inner)
        .unwrap_or_default();
    assert!(dynamic.contains("<context_files path=\"AGENTS.md\">"));
    assert!(dynamic.contains("# project rules"));
}

#[test]
fn copy_system_prompt_with_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    let root = common::TempDir::new("copy-prompt-err");
    copy_system_prompt_with_provider(
        &mock,
        &Config::default(),
        root.path(),
        &FormatRegistry::new(),
    );
    assert!(mock.read().unwrap().is_empty());
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_intent_rule_writes_rule_text_to_clipboard() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    copy_intent_rule_with_provider(&mock);
    let clipboard = mock.read().unwrap();
    assert_eq!(clipboard, i18n::t_str("prompt.system.intent-output-rule"));
    assert!(!clipboard.is_empty());
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_intent_rule_with_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    copy_intent_rule_with_provider(&mock);
    assert!(mock.read().unwrap().is_empty());
}

#[tokio::test]
async fn copy_round_result_copies_selected_round() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-valid");
    let session = session_with_round(root.path()).await;
    push_test_input(&["1"]);
    copy_round_result_with_provider(&mock, &session, 100);
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains("hello"));
}

#[tokio::test]
async fn copy_round_result_with_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    let root = common::TempDir::new("copy-err");
    let session = session_with_round(root.path()).await;
    push_test_input(&["1"]);
    copy_round_result_with_provider(&mock, &session, 100);
    assert!(mock.read().unwrap().is_empty());
}

#[test]
fn truncate_preview_lines_keeps_short_text() {
    let text = "a\nb\nc";
    assert_eq!(truncate_preview_lines(text, 10), text);
}

#[test]
fn truncate_preview_lines_caps_long_text() {
    let _locale_lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let text = (1..=15)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let capped = truncate_preview_lines(&text, 10);
    // 10 kept lines plus the omission note.
    // 保留 10 行加上省略说明。
    assert_eq!(capped.lines().count(), 11);
    assert!(capped.contains("5 lines omitted"));
}

#[tokio::test]
async fn copy_preview_is_indented_and_collapsed() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-preview-indent");
    let session = session_with_round(root.path()).await;
    let _style_lock = STYLE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _locale_lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    manualaid_cli::style::set_enabled(false);
    i18n::set_locale("en");
    push_test_input(&["1"]);
    copy_round_result_with_provider(&mock, &session, 100);
    let output = _capture.text();
    // Every preview line is indented by two spaces (the tool line
    // template already carries its own two leading spaces), and the
    // tool name is bracketed exactly once by the template.
    // 预览每行缩进两个空格（工具行模板本身已带两个前导空格），工具名
    // 只由模板加一次方括号。
    assert!(output.contains("  Round 1 of 1"));
    assert!(output.contains("    [read]"));
    assert!(!output.contains("[[read]]"));
    assert!(output.contains("success  exec"));
    assert!(output.contains("  hello"));
    manualaid_cli::style::set_enabled(false);
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
    let exec = executor(root.path());
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
        &read_call(root.path()),
        50000,
    )
    .await;
    let stats = session.latest(1).expect("round recorded").stats;
    assert!(stats.total_tokens > 0);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn show_tool_history_lists_newest_first() {
    let _capture = manualaid_cli::console::capture();
    let _style_lock = STYLE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _locale_lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    manualaid_cli::style::set_enabled(false);
    i18n::set_locale("en");
    let root = common::TempDir::new("history");
    let mut session = SessionLog::new();
    let stats = RoundStats {
        total_tokens: 100,
        parse_duration_ms: 10,
        audit_duration_ms: 20,
        total_execution_duration_ms: 30,
    };
    add_round(root.path(), &mut session, stats).await;
    add_round(root.path(), &mut session, stats).await;
    show_tool_history(&session);
    let output = _capture.text();
    let newest = output.find("Round 1 of 2").expect("newest header");
    let oldest = output.find("Round 2 of 2").expect("oldest header");
    assert!(newest < oldest);
    assert!(output.contains("[read]"));
    assert!(output.contains("success"));
    // Title line shows the session totals: 2 rounds of 100 tokens and
    // 60 ms each. 标题行显示会话总计：2 轮 × 100 tokens、60 ms。
    assert!(output.contains("200 tokens"));
    assert!(output.contains("120.000000 ms"));
    manualaid_cli::style::set_enabled(false);
}
