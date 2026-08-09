//! Integration tests for the audit layer: path boundaries, command
//! whitelist, dangerous patterns, session modes and the approval queue.
//! 审计层集成测试：路径边界、命令白名单、危险模式、会话模式与审批队列。

use indexmap::IndexMap;
use manualaid_core::audit::{AuditDecision, Auditor, SessionMode};
use manualaid_core::tools::ToolKind;
use serde_json::Value;

fn params(pairs: &[(&str, &str)]) -> IndexMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
        .collect()
}

#[test]
fn read_inside_workspace_is_allowed() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root.clone());
    let file = root.join("a.txt");
    let decisions = auditor.check(
        &params(&[("file_path", file.to_str().unwrap())]),
        ToolKind::Read,
    );
    assert!(decisions.is_empty());
}

#[test]
fn read_outside_workspace_needs_approval() {
    let root = std::env::temp_dir().join("manualaid-ws");
    let auditor = Auditor::new(root);
    let decisions = auditor.check(
        &params(&[("file_path", "C:/outside/a.txt")]),
        ToolKind::Read,
    );
    assert_eq!(decisions.len(), 1);
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn write_inside_workspace_needs_approval_in_manual_mode() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root.clone());
    let file = root.join("new.txt");
    let decisions = auditor.check(
        &params(&[("file_path", file.to_str().unwrap()), ("content", "x")]),
        ToolKind::Write,
    );
    assert_eq!(decisions.len(), 1);
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn write_inside_workspace_is_allowed_in_accept_edit_mode() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root.clone()).with_mode(SessionMode::AcceptEdit);
    let file = root.join("new.txt");
    let decisions = auditor.check(
        &params(&[("file_path", file.to_str().unwrap()), ("content", "x")]),
        ToolKind::Write,
    );
    assert!(decisions.is_empty());
}

#[test]
fn relative_paths_resolve_against_workspace() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root.clone());
    let decisions = auditor.check(&params(&[("file_path", "sub/file.txt")]), ToolKind::Read);
    assert!(decisions.is_empty());
}

#[test]
fn whitelisted_command_is_allowed() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git status".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "git status"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(decisions.is_empty());
}

#[test]
fn non_whitelisted_command_needs_approval() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root);
    let decisions = auditor.check(
        &params(&[("command", "git push"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn whitelisted_base_command_with_chaining_needs_approval() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "git status; rm -rf /"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::Denied(_)));
}

#[test]
fn dangerous_command_is_denied() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root);
    let decisions = auditor.check(
        &params(&[("command", "echo hi; rm -rf /"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::Denied(_)));
}

#[test]
fn content_params_are_allowed() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root);
    let decisions = auditor.check(
        &params(&[
            ("file_path", "/tmp/x"),
            ("old_string", "secret"),
            ("new_string", "public"),
        ]),
        ToolKind::Edit,
    );
    // Only the write path triggers approval; content params never do.
    assert!(decisions.iter().all(|(name, _)| name == "file_path"));
}

#[test]
fn build_approval_queue_flattens_only_needs_approval() {
    let results = vec![
        (
            "write".to_string(),
            vec![(
                "file_path".to_string(),
                AuditDecision::NeedsApproval("outside".into()),
            )],
        ),
        (
            "shell".to_string(),
            vec![
                (
                    "command".to_string(),
                    AuditDecision::Denied("dangerous".into()),
                ),
                ("description".to_string(), AuditDecision::Allowed),
            ],
        ),
    ];
    let queue = Auditor::build_approval_queue(&results);
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].tool_name, "write");
    assert_eq!(queue[0].param_name, "file_path");
}

#[test]
fn exempt_paths_do_not_auto_allow_outside_writes() {
    let root = std::env::temp_dir().join("manualaid-audit-ws");
    let exempt = std::env::temp_dir().join("manualaid-audit-exempt");
    std::fs::create_dir_all(&exempt).unwrap();
    let auditor = Auditor::new(root).with_exempt_paths(vec![exempt.clone()]);
    let file = exempt.join("out.txt");
    let decisions = auditor.check(
        &params(&[("file_path", file.to_str().unwrap()), ("content", "x")]),
        ToolKind::Write,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
    let _ = std::fs::remove_dir_all(&exempt);
}

#[test]
fn non_string_params_are_audited_as_their_text_form() {
    let root = std::env::temp_dir().join("manualaid-audit-ws");
    let auditor = Auditor::new(root);
    let mut parameters = params(&[("file_path", "/outside/x.txt")]);
    parameters.insert("content".to_string(), Value::Bool(true));
    let decisions = auditor.check(&parameters, ToolKind::Write);
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}
