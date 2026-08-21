//! Integration tests for the executor pipeline: routing, validation,
//! auditing, masking restore/re-sanitize, pre-check and post-processing.
//! 执行器管线集成测试：路由、参数校验、审计、掩码还原/重新净化、预检与
//! 后处理。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;
use manualaid_core::audit::{AuditDecision, Auditor};
use manualaid_core::executor::Executor;
use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::ToolCallFormat;
use serde_json::Value;

fn temp_file(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-core-exec-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

fn call(tool_name: &str, pairs: &[(&str, &str)]) -> ParsedToolCall {
    ParsedToolCall {
        tool_name: tool_name.to_string(),
        params: pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
            .collect::<IndexMap<_, _>>(),
        format: ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    }
}

fn executor(root: &std::path::Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(manualaid_core::audit::SessionMode::AcceptEdit),
        Arc::new(None),
    )
}

#[tokio::test]
async fn unknown_tool_fails_without_panicking() {
    let result = executor(&std::env::temp_dir())
        .execute(call("nope", &[]))
        .await;
    assert!(!result.success);
    assert!(result.output.contains("Unknown tool"));
    assert!(result.params_summary.contains('{'));
}

#[tokio::test]
async fn missing_required_param_is_reported() {
    let result = executor(&std::env::temp_dir())
        .execute(call("read", &[]))
        .await;
    assert!(!result.success);
    assert!(result.output.contains("file_path"));
}

#[tokio::test]
async fn hard_denial_blocks_execution() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root.clone());
    let result = Executor::new(auditor, Arc::new(None))
        .execute(call(
            "shell",
            &[("command", "rm -rf /"), ("description", "x")],
        ))
        .await;
    assert!(!result.success);
    assert!(result.output.contains("denied by audit"));
}

#[tokio::test]
async fn read_inside_workspace_executes_immediately() {
    let path = temp_file("exec-read");
    std::fs::write(&path, "exec content").unwrap();
    let root = path.parent().unwrap();
    let result = executor(root)
        .execute(call("read", &[("file_path", path.to_str().unwrap())]))
        .await;
    assert!(result.success);
    assert_eq!(result.output, "exec content");
    assert!(result.read_only);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn masked_placeholders_are_restored_before_execution() {
    let path = temp_file("exec-mask");
    std::fs::write(&path, "masked [PRV_EMAIL_1]").unwrap();
    let mut mapping = HashMap::new();
    mapping.insert("[PRV_EMAIL_1]".to_string(), "alice@example.com".to_string());
    let root = path.parent().unwrap();
    let executor = Executor::new(Auditor::new(root.to_path_buf()), Arc::new(Some(mapping)));
    let result = executor
        .execute(call("read", &[("file_path", path.to_str().unwrap())]))
        .await;
    assert!(result.success);
    assert_eq!(result.output, "masked [PRV_EMAIL_1]");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn pre_check_rejects_failing_edit_before_approval_queue() {
    let root = std::env::temp_dir();
    let result = executor(&root)
        .pre_check(&call(
            "edit",
            &[
                ("file_path", "/tmp/no-such-file.txt"),
                ("old_string", "x"),
                ("new_string", "y"),
            ],
        ))
        .await;
    assert!(result.is_some());
    assert!(!result.unwrap().success);
}

#[tokio::test]
async fn pre_check_returns_none_for_valid_call() {
    let path = temp_file("exec-precheck");
    std::fs::write(&path, "old text").unwrap();
    let root = path.parent().unwrap();
    let pre = executor(root)
        .pre_check(&call(
            "edit",
            &[
                ("file_path", path.to_str().unwrap()),
                ("old_string", "old"),
                ("new_string", "new"),
            ],
        ))
        .await;
    assert!(pre.is_none());
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn pre_check_rejects_read_of_directory_before_approval_queue() {
    let dir = std::env::temp_dir().join(format!(
        "manualaid-core-exec-read-dir-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let result = executor(&std::env::temp_dir())
        .pre_check(&call("read", &[("file_path", dir.to_str().unwrap())]))
        .await;
    let result = result.expect("directory read is a guaranteed failure");
    assert!(!result.success);
    assert!(result.output.contains("file_path"));
    assert!(result.output.contains("directory"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pre_check_rejects_unreadable_read_path() {
    let result = executor(&std::env::temp_dir())
        .pre_check(&call("read", &[("file_path", "Z:/definitely/missing.txt")]))
        .await;
    let result = result.expect("missing read path is a guaranteed failure");
    assert!(!result.success);
    assert!(result.output.contains("cannot read file"));
}

#[tokio::test]
async fn pre_check_passes_for_readable_read_path() {
    let file = temp_file("exec-read-precheck");
    std::fs::write(&file, "readable").unwrap();
    let result = executor(&std::env::temp_dir())
        .pre_check(&call("read", &[("file_path", file.to_str().unwrap())]))
        .await;
    assert!(result.is_none());
    let _ = std::fs::remove_file(&file);
}

#[tokio::test]
async fn pre_check_reports_missing_read_file_path() {
    let executor = executor(&std::env::temp_dir());
    let result = executor.pre_check(&call("read", &[])).await;
    let result = result.expect("missing read path is a guaranteed failure");
    assert!(!result.success);
    assert!(result.output.contains("Missing required parameter"));
}

#[tokio::test]
async fn pre_check_skips_tools_without_readability_validation() {
    let executor = executor(&std::env::temp_dir());
    let result = executor
        .pre_check(&call(
            "shell",
            &[("command", "echo hi"), ("description", "x")],
        ))
        .await;
    assert!(result.is_none());
}

#[tokio::test]
async fn audit_reports_needs_approval_without_executing() {
    let root = std::env::temp_dir().join("manualaid-exec-ws");
    let auditor = Auditor::new(root);
    let executor = Executor::new(auditor, Arc::new(None));
    let decisions = executor.audit(&call(
        "write",
        &[("file_path", "C:/outside/x"), ("content", "y")],
    ));
    assert!(
        decisions
            .iter()
            .any(|(_, decision)| matches!(decision, AuditDecision::NeedsApproval(_)))
    );
}

#[tokio::test]
async fn empty_output_is_substituted() {
    let path = temp_file("exec-empty");
    std::fs::write(&path, "").unwrap();
    let root = path.parent().unwrap();
    let result = executor(root)
        .execute(call("read", &[("file_path", path.to_str().unwrap())]))
        .await;
    assert!(result.success);
    assert!(result.is_fallback);
    assert!(result.output.contains("no output"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn tool_count_matches_builtin_tools() {
    let executor = executor(&std::env::temp_dir());
    assert_eq!(
        executor.tool_count(),
        manualaid_core::tools::all_tools().len()
    );
    assert!(executor.find_tool("read").is_some());
    assert!(executor.find_tool("nope").is_none());
}

#[test]
fn audit_of_unknown_tool_is_empty() {
    let executor = executor(&std::env::temp_dir());
    let decisions = executor.audit(&call("bogus-tool", &[]));
    assert!(decisions.is_empty());
}

#[tokio::test]
async fn pre_check_reports_missing_required_params() {
    let executor = executor(&std::env::temp_dir());
    let result = executor.pre_check(&call("write", &[])).await;
    let result = result.expect("missing params are a guaranteed failure");
    assert!(!result.success);
    assert!(result.output.contains("Missing required parameter"));
}

#[tokio::test]
async fn pre_check_passes_when_params_are_valid() {
    let root = std::env::temp_dir().join("manualaid-exec-prec");
    let file = temp_file("prec");
    std::fs::write(&file, "old").unwrap();
    let executor = executor(&root);
    let result = executor
        .pre_check(&call(
            "edit",
            &[
                ("file_path", file.to_str().unwrap()),
                ("old_string", "old"),
                ("new_string", "new"),
            ],
        ))
        .await;
    assert!(result.is_none());
    let _ = std::fs::remove_file(&file);
}
