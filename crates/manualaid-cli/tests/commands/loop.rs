//! Integration tests for the Agent Copy-Paste Loop helpers: round execution
//! with approval, menu rendering, index parsing and format cycling.
//! Agent Copy-Paste Loop 辅助函数的集成测试：带审批的轮次执行、菜单渲染、
//! 索引解析与格式循环切换。

use std::sync::Arc;

use manualaid_cli::commands::loop_cli::{
    Approval, LoopOptions, cycle_format, cycle_lang, execute_round_with_approval,
    format_round_summary, parse_round_index, render_config_menu, render_menu,
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
    let _capture = manualaid_cli::console::capture();
    let root = std::env::temp_dir().join("manualaid-loop-ws");
    // An absolute path next to the workspace root stays outside the
    // workspace on both Windows and Linux (`C:/outside/x.txt` is relative
    // on Linux and would resolve inside the workspace instead).
    // 工作区根目录旁的绝对路径在 Windows 与 Linux 上都位于工作区之外
    // （`C:/outside/x.txt` 在 Linux 上是相对路径，会被解析进工作区）。
    let outside = root
        .parent()
        .expect("temp dir has a parent")
        .join("manualaid-loop-outside.txt");
    let outside = outside.to_string_lossy();
    let registry = FormatRegistry::new();
    let call = format!("<write><file_path>{outside}</file_path><content>y</content></write>");
    let (_, results) =
        execute_round_with_approval(&executor(&root), &registry, &call, |_| Approval::Deny)
            .await
            .unwrap();
    assert!(!results[0].success);
    // The audit reason embeds the original path, so this assertion holds in
    // every locale without pinning the process-wide i18n setting.
    // 审计原因包含原始路径，断言在所有语言下都成立，无需固定进程级 locale。
    assert!(results[0].output.contains(&*outside));
}

#[tokio::test]
async fn deny_with_text_becomes_the_tool_result() {
    let _capture = manualaid_cli::console::capture();
    let root = std::env::temp_dir().join("manualaid-loop-ws");
    let outside = root
        .parent()
        .expect("temp dir has a parent")
        .join("manualaid-loop-outside-text.txt");
    let outside = outside.to_string_lossy();
    let registry = FormatRegistry::new();
    let call = format!("<write><file_path>{outside}</file_path><content>y</content></write>");
    let (_, results) = execute_round_with_approval(&executor(&root), &registry, &call, |_| {
        Approval::DenyWithText("use the read tool instead".to_string())
    })
    .await
    .unwrap();
    assert!(!results[0].success);
    assert!(results[0].output.contains("read tool"));
}

#[tokio::test]
async fn mixed_approve_and_deny_in_one_round() {
    let _capture = manualaid_cli::console::capture();
    let ws = std::env::temp_dir().join(format!("manualaid-loop-mixed-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    let registry = FormatRegistry::new();
    let calls = format!(
        "<write><file_path>{}/a.txt</file_path><content>A</content></write>\
         <write><file_path>{}/b.txt</file_path><content>B</content></write>",
        ws.display(),
        ws.display()
    );
    let executor = Executor::new(
        Auditor::new(ws.clone()).with_mode(SessionMode::Manual),
        Arc::new(None),
    );
    let mut decisions = 0;
    let (_, results) = execute_round_with_approval(&executor, &registry, &calls, |_| {
        decisions += 1;
        if decisions == 1 {
            Approval::Approve
        } else {
            Approval::Deny
        }
    })
    .await
    .unwrap();
    assert_eq!(decisions, 2);
    assert!(results[0].success);
    assert!(
        std::fs::read_to_string(ws.join("a.txt"))
            .unwrap()
            .contains("A")
    );
    assert!(!results[1].success);
    assert!(!ws.join("b.txt").exists());
}

#[tokio::test]
async fn pre_failed_call_skips_approval_while_others_are_asked() {
    let _capture = manualaid_cli::console::capture();
    let ws = std::env::temp_dir().join(format!("manualaid-loop-prefail-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    let registry = FormatRegistry::new();
    let calls = format!(
        "<edit><file_path>{}/missing.txt</file_path><old_string>a</old_string><new_string>b</new_string></edit>\
         <write><file_path>{}/kept.txt</file_path><content>B</content></write>",
        ws.display(),
        ws.display()
    );
    let executor = Executor::new(
        Auditor::new(ws.clone()).with_mode(SessionMode::Manual),
        Arc::new(None),
    );
    let mut decisions = 0;
    let (_, results) = execute_round_with_approval(&executor, &registry, &calls, |_| {
        decisions += 1;
        Approval::Approve
    })
    .await
    .unwrap();
    assert_eq!(decisions, 1);
    assert!(!results[0].success);
    assert!(results[1].success);
    assert!(ws.join("kept.txt").exists());
}

#[tokio::test]
async fn malformed_call_is_a_parse_error() {
    let registry = FormatRegistry::new();
    let result = execute_round_with_approval(
        &executor(&std::env::temp_dir()),
        &registry,
        "<write><file_path>Z:/a.txt</file_path>",
        |_| Approval::Approve,
    )
    .await;
    assert!(result.is_err());
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

#[tokio::test]
async fn accept_edit_auto_approves_workspace_write() {
    let _capture = manualaid_cli::console::capture();
    let ws =
        std::env::temp_dir().join(format!("manualaid-loop-accept-edit-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    let registry = FormatRegistry::new();
    let calls = format!(
        "<write><file_path>{}/auto.txt</file_path><content>A</content></write>",
        ws.display()
    );
    let mut decisions = 0;
    let (_, results) = execute_round_with_approval(&executor(&ws), &registry, &calls, |_| {
        decisions += 1;
        Approval::Deny
    })
    .await
    .unwrap();
    assert_eq!(decisions, 0);
    assert!(results[0].success);
    assert!(ws.join("auto.txt").exists());
}

#[tokio::test]
async fn manual_mode_asks_before_workspace_write() {
    let _capture = manualaid_cli::console::capture();
    let ws = std::env::temp_dir().join(format!("manualaid-loop-manual-{}", std::process::id()));
    std::fs::create_dir_all(&ws).unwrap();
    let registry = FormatRegistry::new();
    let calls = format!(
        "<write><file_path>{}/asked.txt</file_path><content>A</content></write>",
        ws.display()
    );
    let executor = Executor::new(
        Auditor::new(ws.clone()).with_mode(SessionMode::Manual),
        Arc::new(None),
    );
    let mut decisions = 0;
    let (_, results) = execute_round_with_approval(&executor, &registry, &calls, |_| {
        decisions += 1;
        Approval::Approve
    })
    .await
    .unwrap();
    assert_eq!(decisions, 1);
    assert!(results[0].success);
    assert!(ws.join("asked.txt").exists());
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
