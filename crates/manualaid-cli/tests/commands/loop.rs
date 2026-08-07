//! Integration tests for the Agent Copy-Paste Loop helpers: round execution
//! with approval, menu rendering, index parsing and format cycling.
//! Agent Copy-Paste Loop 辅助函数的集成测试：带审批的轮次执行、菜单渲染、
//! 索引解析与格式循环切换。

use std::sync::Arc;

use manualaid_cli::commands::loop_cli::{
    Approval, cycle_format, cycle_lang, execute_round_with_approval, format_round_summary,
    parse_round_index, render_config_menu, render_menu, LoopOptions,
};
use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_core::tools::ToolResult;
use manualaid_ws::config::Config;

fn executor(root: &std::path::Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(SessionMode::AcceptEdit),
        Arc::new(None),
    )
}

#[tokio::test]
async fn round_with_no_calls_is_an_error() {
    let registry = FormatRegistry::new();
    let result = execute_round_with_approval(
        &executor(&std::env::temp_dir()),
        &registry,
        "plain text",
        |_| Approval::Approve,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn approved_round_executes_tools() {
    let registry = FormatRegistry::new();
    let (calls, results) = execute_round_with_approval(
        &executor(&std::env::temp_dir()),
        &registry,
        "<read><file_path>C:/windows/win.ini</file_path></read>",
        |_| Approval::Approve,
    )
    .await
    .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(results.len(), 1);
    // On Windows win.ini exists; on other platforms the tool still returns
    // a structured result either way.
    assert_eq!(results[0].tool_name, "read");
}

#[tokio::test]
async fn denied_round_returns_failure_with_reason() {
    let root = std::env::temp_dir().join("manualaid-loop-ws");
    let registry = FormatRegistry::new();
    let (_, results) = execute_round_with_approval(
        &executor(&root),
        &registry,
        "<write><file_path>C:/outside/x.txt</file_path><content>y</content></write>",
        |_| Approval::Deny,
    )
    .await
    .unwrap();
    assert!(!results[0].success);
    assert!(results[0].output.contains("denied"));
}

#[tokio::test]
async fn deny_with_text_becomes_the_tool_result() {
    let root = std::env::temp_dir().join("manualaid-loop-ws");
    let registry = FormatRegistry::new();
    let (_, results) = execute_round_with_approval(
        &executor(&root),
        &registry,
        "<write><file_path>C:/outside/x.txt</file_path><content>y</content></write>",
        |_| Approval::DenyWithText("use the read tool instead".to_string()),
    )
    .await
    .unwrap();
    assert!(!results[0].success);
    assert!(results[0].output.contains("read tool"));
}

#[tokio::test]
async fn pre_failed_calls_never_ask_for_approval() {
    let registry = FormatRegistry::new();
    let mut decisions = 0;
    let (_, results) = execute_round_with_approval(
        &executor(&std::env::temp_dir()),
        &registry,
        "<edit><file_path>Z:/missing/file.txt</file_path><old_string>a</old_string><new_string>b</new_string></edit>",
        |_| {
            decisions += 1;
            Approval::Approve
        },
    )
    .await
    .unwrap();
    assert_eq!(decisions, 0);
    assert!(!results[0].success);
}

#[test]
fn menu_contains_all_options() {
    let menu = render_menu();
    for label in [
        "cli.loop.menu_title",
        "cli.loop.menu_generate",
        "cli.loop.menu_paste",
        "cli.loop.menu_copy",
        "cli.loop.menu_exit",
    ] {
        assert!(menu.contains(&i18n::t_str(label)));
    }
}

#[test]
fn config_menu_shows_current_states() {
    let config = Config {
        shell: false,
        ..Config::default()
    };
    let menu = render_config_menu(&config, &LoopOptions::default());
    assert!(menu.contains(&i18n::t_str("cli.config.disabled")));
    assert!(menu.contains(&i18n::t_str("cli.config.enabled")));
}

#[test]
fn round_summary_marks_success_and_failure() {
    let summary = format_round_summary(&[
        ToolResult::success("read", "data", true),
        ToolResult::failure("edit", "boom"),
    ]);
    assert!(summary.contains("[read]"));
    assert!(summary.contains("data"));
    assert!(summary.contains("[edit]"));
}

#[test]
fn format_and_lang_cycling_helpers() {
    assert_eq!(cycle_format("auto"), "xml");
    assert_eq!(cycle_format("json-codeblock"), "auto");
    assert_eq!(cycle_lang("en"), "zh-CN");
    assert_eq!(cycle_lang("zh-CN"), "en");
}

#[test]
fn round_index_parsing() {
    assert_eq!(parse_round_index("", 5), Some(1));
    assert_eq!(parse_round_index("3", 5), Some(3));
    assert_eq!(parse_round_index("6", 5), None);
    assert_eq!(parse_round_index("x", 5), None);
}
