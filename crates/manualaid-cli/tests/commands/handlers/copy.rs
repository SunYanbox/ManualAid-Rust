//! Tests for the clipboard-copying handler functions.
//! 剪贴板复制处理器函数的测试。

use crate::LOCALE_LOCK;
use crate::common;
use manualaid_cli::commands::loop_cli::{
    copy_compressed_fence_with_provider, copy_compressed_session_prompt_with_provider,
    copy_context_with_provider, copy_enabled_tools_with_provider, copy_intent_rule_with_provider,
    copy_line_ending_rule_with_provider, copy_plan_mode_rule_with_provider, copy_round_result,
    copy_round_result_with_provider, copy_switch_mode_rule_with_provider,
    copy_system_prompt_with_provider, copy_task_planning_rule_with_provider,
    copy_tool_format_with_provider,
};
use manualaid_core::clipboard::{ClipboardProvider, MockClipboard};
use manualaid_core::parser::FormatRegistry;
use manualaid_ws::config::Config;

/// Assert that the checkpoint sections appear in `text` in the given order, so
/// a dropped or reordered section fails the test instead of passing unnoticed.
/// 断言各检查点小节按给定顺序出现在 `text` 中，使缺失或错序的小节能被测试
/// 发现，而不是静默通过。
fn assert_sections_in_order(text: &str, sections: &[&str]) {
    let mut rest = text;
    for section in sections {
        let at = rest
            .find(section)
            .unwrap_or_else(|| panic!("section missing or out of order: {section}"));
        rest = &rest[at + section.len()..];
    }
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
    )
    .unwrap();
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains("\"tool_use\": \"read\""));
}

#[test]
fn copy_system_prompt_includes_selected_context_files() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-prompt-context");
    std::fs::write(root.path().join("AGENTS.md"), "# project rules").unwrap();
    copy_system_prompt_with_provider(
        &mock,
        &Config::default(),
        root.path(),
        &FormatRegistry::new(),
    )
    .unwrap();
    let clipboard = mock.read().unwrap();
    let reminder = clipboard
        .split_once("</system_prompt>")
        .and_then(|(_, rest)| rest.split_once("<system-reminder>"))
        .map(|(_, inner)| {
            inner
                .split_once("</system-reminder>")
                .map(|(inner, _)| inner)
                .unwrap_or_default()
        })
        .unwrap_or_default();
    assert!(reminder.contains("[workspace instructions AGENTS.md]"));
    assert!(reminder.contains("# project rules"));
}

#[test]
fn copy_context_with_no_files_prints_notice_and_keeps_clipboard_empty() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-context-empty");
    copy_context_with_provider(&mock, root.path()).unwrap();
    assert!(mock.read().unwrap().is_empty());
    assert!(
        _capture
            .text()
            .contains(&i18n::t_str("cli.message.no_context_files"))
    );
}

#[test]
fn copy_system_prompt_with_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    let root = common::TempDir::new("copy-prompt-err");
    let err = copy_system_prompt_with_provider(
        &mock,
        &Config::default(),
        root.path(),
        &FormatRegistry::new(),
    )
    .unwrap_err();
    assert_eq!(err, "mock write failure");
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
    copy_intent_rule_with_provider(&mock).unwrap();
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
    let err = copy_intent_rule_with_provider(&mock).unwrap_err();
    assert_eq!(err, "mock write failure");
    assert!(mock.read().unwrap().is_empty());
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_tool_format_writes_current_format_to_clipboard() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    let registry = FormatRegistry::new();
    copy_tool_format_with_provider(&mock, &registry).unwrap();
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains(&i18n::t_str("cli.prompt.func_calls_notes")));
    assert!(clipboard.contains("\"tool_use\": \"read\""));
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_enabled_tools_writes_full_list_with_reminder() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    let registry = FormatRegistry::new();
    copy_enabled_tools_with_provider(&mock, &Config::default(), &registry).unwrap();
    let clipboard = mock.read().unwrap();
    assert!(clipboard.starts_with("<system-reminder>\n"));
    assert!(clipboard.ends_with("</system-reminder>"));
    assert!(clipboard.contains("## read"));
    assert!(clipboard.contains("## edit"));
    assert!(clipboard.contains("## write"));
    assert!(clipboard.contains("## shell"));
    assert!(clipboard.contains("**Parameters:**"));
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_enabled_tools_respects_disabled_switch() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    let config = Config {
        shell: false,
        ..Config::default()
    };
    let registry = FormatRegistry::new();
    copy_enabled_tools_with_provider(&mock, &config, &registry).unwrap();
    let clipboard = mock.read().unwrap();
    assert!(!clipboard.contains("## shell"));
    assert!(clipboard.contains("## read"));
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_rule_prompts_write_expected_text_to_clipboard() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let cases = [
        (
            copy_line_ending_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
            i18n::t_str("prompt.copy.line-ending-rule"),
        ),
        (
            copy_plan_mode_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
            i18n::t_str("prompt.copy.plan-mode-rule"),
        ),
        (
            copy_switch_mode_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
            i18n::t_str("prompt.copy.switch-mode-rule"),
        ),
        (
            copy_task_planning_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
            i18n::t_str("prompt.copy.task-planning-rule"),
        ),
    ];
    for (copy_fn, expected) in cases {
        let mock = MockClipboard::new();
        copy_fn(&mock).unwrap();
        assert_eq!(mock.read().unwrap(), expected);
    }
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_rule_prompts_with_write_error_do_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let copy_fns = [
        copy_line_ending_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
        copy_plan_mode_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
        copy_switch_mode_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
        copy_task_planning_rule_with_provider as fn(&MockClipboard) -> Result<(), String>,
    ];
    for copy_fn in copy_fns {
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");
        let err = copy_fn(&mock).unwrap_err();
        assert_eq!(err, "mock write failure");
        assert!(mock.read().unwrap().is_empty());
    }
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_compressed_session_prompt_writes_verbatim_text() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    copy_compressed_session_prompt_with_provider(&mock).unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.starts_with("<system-reminder>\n"));
    assert!(copied.ends_with("\n</system-reminder>"));
    assert!(copied.contains("resume checkpoint"));
    assert!(copied.contains("`<manualaid-checkpoint>`"));
    assert_sections_in_order(
        &copied,
        &[
            "## Task Goal and Current Stage",
            "## Files and Code Involved",
            "## Data Needed to Resume Work",
            "## Actions Already Taken",
            "## Constraints and Prohibitions",
            "## Open Items and Resume Point",
        ],
    );
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_compressed_fence_writes_verbatim_template() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let mock = MockClipboard::new();
    copy_compressed_fence_with_provider(&mock).unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.starts_with("This is a resume checkpoint"));
    assert!(copied.contains("<manualaid-checkpoint>"));
    assert!(copied.contains("</manualaid-checkpoint>"));
    assert!(!copied.contains("<system-reminder>"));
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_compressed_fence_writes_localized_chinese_text() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("zh-CN");
    let mock = MockClipboard::new();
    copy_compressed_fence_with_provider(&mock).unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.contains("续作检查点"));
    assert!(copied.contains("<manualaid-checkpoint>"));
    assert!(!copied.contains("<system-reminder>"));
}

#[test]
#[allow(clippy::await_holding_lock)]
fn copy_compressed_session_prompt_writes_localized_chinese_text() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("zh-CN");
    let mock = MockClipboard::new();
    copy_compressed_session_prompt_with_provider(&mock).unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.starts_with("<system-reminder>\n"));
    assert!(copied.ends_with("\n</system-reminder>"));
    assert!(copied.contains("续作检查点"));
    assert!(copied.contains("`<manualaid-checkpoint>`"));
    assert_sections_in_order(
        &copied,
        &[
            "## 任务目标与当前阶段",
            "## 涉及的文件与代码",
            "## 继续干活所需的数据",
            "## 已执行的处置",
            "## 约束与禁令",
            "## 未完事项与接续点",
        ],
    );
}

#[tokio::test]
async fn copy_round_result_copies_selected_round() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    let root = common::TempDir::new("copy-valid");
    let session = super::session_with_round(root.path()).await;
    manualaid_cli::commands::loop_cli::push_test_input(&[super::LATEST_ROUND_INDEX]);
    copy_round_result_with_provider(&mock, root.path(), &session, 100);
    let clipboard = mock.read().unwrap();
    assert!(clipboard.contains("hello"));
}

#[tokio::test]
async fn copy_round_result_with_write_error_does_not_panic() {
    let _capture = manualaid_cli::console::capture();
    let mock = MockClipboard::new();
    mock.set_write_error("mock write failure");
    let root = common::TempDir::new("copy-err");
    let session = super::session_with_round(root.path()).await;
    manualaid_cli::commands::loop_cli::push_test_input(&[super::LATEST_ROUND_INDEX]);
    copy_round_result_with_provider(&mock, root.path(), &session, 100);
    assert!(mock.read().unwrap().is_empty());
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
    let session = super::session_with_round(root.path()).await;
    manualaid_cli::commands::loop_cli::push_test_input(&[super::OUT_OF_RANGE_INDEX]);
    copy_round_result(root.path(), &session, 100);
    assert!(_capture.text().contains("Invalid round index"));
}

#[test]
fn copy_round_result_without_rounds_prints_notice() {
    let _capture = manualaid_cli::console::capture();
    let _lock = LOCALE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let session = manualaid_ws::session::SessionLog::new();
    let root = common::TempDir::new("copy-empty");
    copy_round_result(root.path(), &session, 100);
    assert!(
        _capture
            .text()
            .contains("No parse rounds available to copy")
    );
}
