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
    // An absolute path next to the workspace root stays outside the
    // workspace on both Windows and Linux (`C:/outside/a.txt` is relative
    // on Linux and would resolve inside the workspace instead).
    // 工作区根目录旁的绝对路径在 Windows 与 Linux 上都位于工作区之外
    // （`C:/outside/a.txt` 在 Linux 上是相对路径，会被解析进工作区）。
    let outside = root
        .parent()
        .expect("temp dir has a parent")
        .join("outside.txt");
    let auditor = Auditor::new(root);
    let decisions = auditor.check(
        &params(&[("file_path", outside.to_str().unwrap())]),
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

#[test]
fn wildcard_whitelist_command_is_allowed() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git log *".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "git log --oneline -5"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(decisions.is_empty());
}

#[test]
fn wildcard_whitelist_matches_zero_or_more_characters() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git log *".to_string()]);
    for command in ["git log --oneline", "git log --author=Alice -n 3"] {
        let decisions = auditor.check(
            &params(&[("command", command), ("description", "x")]),
            ToolKind::Shell,
        );
        assert!(decisions.is_empty(), "{command} must be whitelisted");
    }
    // The literal space before `*` is part of the pattern, so the bare
    // command is not covered by `git log *`.
    // `*` 前的空格属于模式字面量，因此裸命令不在 `git log *` 的覆盖范围内。
    let decisions = auditor.check(
        &params(&[("command", "git log"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn exact_whitelist_does_not_match_extra_arguments() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git log".to_string()]);
    let allowed = auditor.check(
        &params(&[("command", "git log"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(allowed.is_empty());
    let decisions = auditor.check(
        &params(&[("command", "git log --oneline"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn config_commands_merge_with_default_whitelist() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git log *".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "git status"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(
        decisions.is_empty(),
        "built-in default must stay whitelisted"
    );
}

#[test]
fn default_whitelist_covers_platform_listing_command() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root);
    #[cfg(windows)]
    let listing = "dir";
    #[cfg(not(windows))]
    let listing = "ls";
    let decisions = auditor.check(
        &params(&[("command", listing), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(decisions.is_empty());
}

#[test]
fn wildcard_whitelist_cannot_match_chained_command() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["git *".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "git status; echo hi"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn dangerous_whitelist_entries_are_detected() {
    for pattern in ["rm -rf /", "rm *", "*", "> *", "mkfs*", "> /dev/sda"] {
        assert!(
            manualaid_core::audit::is_dangerous_allow_command(pattern),
            "{pattern} must be dangerous"
        );
    }
    for pattern in [
        "git status",
        "git log *",
        "gh pr view *",
        "cargo fmt -- --check",
    ] {
        assert!(
            !manualaid_core::audit::is_dangerous_allow_command(pattern),
            "{pattern} must be safe"
        );
    }
}

#[test]
fn sanitize_allow_commands_keeps_order_and_ignores_dangerous() {
    let (kept, ignored) = manualaid_core::audit::sanitize_allow_commands(vec![
        "git log *".to_string(),
        "rm *".to_string(),
        "git status".to_string(),
        "*".to_string(),
    ]);
    assert_eq!(kept, vec!["git log *", "git status"]);
    assert_eq!(ignored, vec!["rm *", "*"]);
}

#[test]
fn auditor_ignores_dangerous_config_commands() {
    let root = std::env::temp_dir();
    let auditor = Auditor::new(root).with_allowed_commands(vec!["rm *".to_string()]);
    let decisions = auditor.check(
        &params(&[("command", "rm file.txt"), ("description", "x")]),
        ToolKind::Shell,
    );
    assert!(matches!(decisions[0].1, AuditDecision::NeedsApproval(_)));
}

#[test]
fn wildcard_match_follows_star_semantics() {
    assert!(manualaid_core::audit::wildcard_match(
        "git log *",
        "git log --oneline"
    ));
    assert!(manualaid_core::audit::wildcard_match("git log*", "git log"));
    assert!(!manualaid_core::audit::wildcard_match(
        "git log *",
        "git log"
    ));
    assert!(manualaid_core::audit::wildcard_match("git *", "git status"));
    assert!(!manualaid_core::audit::wildcard_match(
        "git log *",
        "git push"
    ));
    assert!(manualaid_core::audit::wildcard_match("*", "anything"));
}
