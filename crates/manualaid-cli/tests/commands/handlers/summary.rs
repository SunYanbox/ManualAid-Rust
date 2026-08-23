//! Tests for session summary, tool history and preview truncation.
//! 会话摘要、工具历史与预览截断的测试。

use crate::common;
use crate::{LOCALE_LOCK, STYLE_LOCK};
use manualaid_cli::commands::loop_cli::{
    copy_round_result_with_provider, print_session_summary, push_test_input, show_tool_history,
    truncate_preview_lines,
};
use manualaid_core::clipboard::MockClipboard;
use manualaid_ws::config::Config;
use manualaid_ws::session::{RoundStats, SessionLog};

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
    let session = super::session_with_round(root.path()).await;
    print_session_summary(&Config::default(), &session);
    let text = _capture.text();
    assert!(text.contains("Session summary"));
    assert!(text.contains("Rounds: 1"));
    assert!(text.contains("Tool calls: 1"));
    assert!(text.contains("Enabled tools:"));
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
    let session = super::session_with_round(root.path()).await;
    let _style_lock = STYLE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _locale_lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _style_guard = super::StyleGuard::new();
    manualaid_cli::style::set_enabled(false);
    i18n::set_locale("en");
    push_test_input(&[super::LATEST_ROUND_INDEX]);
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
    let _style_guard = super::StyleGuard::new();
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
    super::add_round(root.path(), &mut session, stats).await;
    super::add_round(root.path(), &mut session, stats).await;
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
}
