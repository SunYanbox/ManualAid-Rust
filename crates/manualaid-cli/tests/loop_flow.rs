//! Console-flow tests for the interactive Agent Copy-Paste Loop: the real
//! `manualaid-cli` binary is spawned as a separate process with scripted
//! stdin, so the menu, tool execution and clipboard flows run in a fresh
//! console process instead of inside the test harness.
//! 交互式 Agent Copy-Paste Loop 的控制台流测试：以独立进程运行真实
//! `manualaid-cli` 二进制并通过脚本化 stdin 驱动，使菜单、工具执行与剪贴板
//! 流程运行在全新的控制台进程中，而非测试进程内部。
mod common;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use manualaid_cli::commands::loop_cli::{Approval, execute_round_with_approval};
use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;

fn read_call(path: &Path) -> String {
    format!("<read><file_path>{}</file_path></read>", path.display())
}

fn config_path(cwd: &Path) -> std::path::PathBuf {
    cwd.join(".ManualAid").join("config.toml")
}

#[tokio::test]
async fn approval_flow_skips_read_of_directory() {
    let root = common::TempDir::new("loop-flow-read-dir");
    let dir = root.path().join("subdir");
    std::fs::create_dir_all(&dir).unwrap();
    let executor = Executor::new(
        Auditor::new(root.path().to_path_buf()).with_mode(SessionMode::AcceptEdit),
        Arc::new(None),
    );
    let registry = FormatRegistry::new();
    let read_call = read_call(&dir);
    let calls = registry.parse(&read_call).unwrap().calls;
    let decide_calls = AtomicUsize::new(0);
    let (_, results, _) = execute_round_with_approval(&executor, &registry, &read_call, |_| {
        decide_calls.fetch_add(1, Ordering::SeqCst);
        Approval::Approve
    })
    .await
    .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(results.len(), 1);
    let result = &results[0];
    assert!(!result.success);
    assert!(result.output.contains("file_path"));
    assert!(result.output.contains("directory"));
    assert_eq!(decide_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn loop_binary_drives_menu_inline_commands_and_typed_round() {
    let dir = common::TempDir::new("loop-flow-menu");
    let home = common::TempDir::new("loop-flow-menu-home");
    std::fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    std::fs::write(
        config_path(dir.path()),
        "[global]\nlang = \"zh-CN\"\ntool_call_format = \"bogus\"\n",
    )
    .unwrap();
    let target = dir.path().join("target.txt");
    std::fs::write(&target, "hello").unwrap();
    let read_call = read_call(&target);
    // Assert on stdout and the config file only; the round result copy is a
    // side effect of a successful round and is not needed to verify the menu
    // flow. This avoids any dependency on the system clipboard.
    // 仅通过 stdout 与配置文件断言；轮次结果复制是成功执行后的副作用，
    // 验证菜单流程无需依赖系统剪贴板。
    let output = common::run_binary_scripted(
        dir.path(),
        Some(home.path()),
        &[],
        &[
            "/tools",
            "/format 2",
            "4",
            "5",
            "9",
            "11",
            "0",
            "3",
            &read_call,
            "/end",
            "n",
            "6",
            "x",
            "0",
        ],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ManualAid CLI Agent Copy-Paste Loop"));
    assert!(stdout.contains("[read]"));
    assert!(stdout.contains("hello"));
    let content = std::fs::read_to_string(config_path(dir.path())).unwrap();
    assert!(content.contains("lang = \"zh-CN\""));
    assert!(content.contains("tool_call_format = \"xml\""));
}

#[test]
fn loop_binary_input_menu_submits_text() {
    let dir = common::TempDir::new("loop-flow-input");
    let home = common::TempDir::new("loop-flow-input-home");
    let target = dir.path().join("target.txt");
    std::fs::write(&target, "hello").unwrap();
    // Use the "input tool call text" menu option (3) instead of the paste
    // option (2): the paste path needs a real system clipboard write before
    // the child starts, while the input path uses stdin only and is stable
    // across headless CI and Windows. The clipboard-backed paste behavior is
    // already covered with MockClipboard by the in-process handler tests.
    // 使用“输入工具调用文本”菜单项（3）而非粘贴项（2）：粘贴路径需要子进程
    // 启动前写真实系统剪贴板，而输入路径仅依赖 stdin，在无头 CI 与 Windows
    // 上都稳定。粘贴的剪贴板行为已由进程内 handler 测试用 MockClipboard 覆盖。
    let output = common::run_binary_scripted(
        dir.path(),
        Some(home.path()),
        &[],
        &["3", &read_call(&target), "/end", "n", "0"],
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[read]"));
    assert!(stdout.contains("hello"));
}

#[test]
fn loop_binary_accept_edit_auto_approves_workspace_write() {
    let dir = common::TempDir::new("loop-flow-accept-edit");
    let home = common::TempDir::new("loop-flow-accept-edit-home");
    let file = dir.path().join("created.txt");
    let write_call = format!(
        "<write><file_path>{}</file_path><content>ok</content></write>",
        file.display()
    );
    let output = common::run_binary_scripted(
        dir.path(),
        Some(home.path()),
        &["--mode", "accept-edit"],
        &["5", "8", "0", "3", &write_call, "/end", "n", "0"],
    );
    assert!(output.status.success());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "ok");
}

#[test]
fn loop_binary_exits_on_zero_with_explicit_and_real_home() {
    let dir = common::TempDir::new("loop-flow-home");
    let home = common::TempDir::new("loop-flow-home-temp");
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["0"]);
    assert!(output.status.success());
    assert!(config_path(dir.path()).is_file());
    // Without a home override the loop falls back to the real user home and
    // still exits cleanly; it only reads the home directory.
    let output = common::run_binary_scripted(dir.path(), None, &[], &["0"]);
    assert!(output.status.success());
}

#[test]
fn loop_binary_reports_invalid_config() {
    let dir = common::TempDir::new("loop-flow-bad-config");
    let home = common::TempDir::new("loop-flow-bad-config-home");
    std::fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    std::fs::write(config_path(dir.path()), "not [valid toml").unwrap();
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["0"]);
    assert!(!output.status.success());
    assert!(!output.stderr.is_empty());
}

#[test]
fn loop_binary_copies_single_context_file_to_clipboard() {
    let dir = common::TempDir::new("loop-flow-ctx-single");
    let home = common::TempDir::new("loop-flow-ctx-single-home");
    std::fs::write(dir.path().join("AGENTS.md"), "# rules").unwrap();
    // Assert the confirmation printed after a successful copy instead of
    // polling the system clipboard: the clipboard owner may not respond
    // while the child is blocked on stdin, which makes clipboard polling
    // flaky on headless CI and on Windows.
    // 通过复制成功后的确认输出断言，而非轮询系统剪贴板：子进程阻塞等待
    // stdin 时剪贴板所有者可能无法响应，导致无头 CI 与 Windows 上轮询
    // 剪贴板不稳定。
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["1", "0"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&i18n::t_str("cli.message.prompt_copied")));
}

#[test]
fn loop_binary_copies_intent_rule_to_clipboard() {
    let dir = common::TempDir::new("loop-flow-intent-rule");
    let home = common::TempDir::new("loop-flow-intent-rule-home");
    // Assert the confirmation printed after a successful copy instead of
    // polling the system clipboard: the clipboard owner may not respond
    // while the child is blocked on stdin, which makes clipboard polling
    // flaky on headless CI and on Windows.
    // 通过复制成功后的确认输出断言，而非轮询系统剪贴板：子进程阻塞等待
    // stdin 时剪贴板所有者可能无法响应，导致无头 CI 与 Windows 上轮询
    // 剪贴板不稳定。
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["8", "0"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&i18n::t_str("cli.message.intent_rule_copied")));
}

#[test]
fn loop_binary_asks_selection_when_multiple_context_files_exist() {
    let dir = common::TempDir::new("loop-flow-ctx-multi");
    let home = common::TempDir::new("loop-flow-ctx-multi-home");
    std::fs::write(dir.path().join("AGENTS.md"), "same").unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "same").unwrap();
    // Drive the selection through stdout instead of polling the system
    // clipboard: the clipboard owner may not respond while the child is
    // blocked on stdin, which makes clipboard polling flaky on headless
    // CI and on Windows.
    // 通过 stdout 驱动选择，而非轮询系统剪贴板：子进程阻塞等待 stdin 时
    // 剪贴板所有者可能无法响应，导致无头 CI 与 Windows 上轮询剪贴板
    // 不稳定。
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["1", "1", "0"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Context files loaded: AGENTS.md"));
    assert!(!stdout.contains("Context files loaded: AGENTS.md, CLAUDE.md"));
    assert!(stdout.contains("(duplicate of AGENTS.md)"));
}

#[test]
fn loop_binary_skips_context_selection_when_auto_load_is_disabled() {
    let dir = common::TempDir::new("loop-flow-ctx-off");
    let home = common::TempDir::new("loop-flow-ctx-off-home");
    std::fs::create_dir_all(dir.path().join(".ManualAid")).unwrap();
    std::fs::write(
        config_path(dir.path()),
        "[global]\ncontext_auto_load = false\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "a").unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "b").unwrap();
    // Assert the successful copy confirmation and that no selection menu
    // appears, instead of polling the system clipboard. The clipboard may
    // not answer while the child waits on stdin, so stdout is the stable
    // side channel here.
    // 断言复制成功确认且未出现选择菜单，而非轮询系统剪贴板。子进程等待
    // stdin 时剪贴板可能无法响应，因此 stdout 是这里的稳定验证渠道。
    let output = common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["1", "0"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&i18n::t_str("cli.message.prompt_copied")));
    assert!(!stdout.contains(&i18n::t_str("cli.context.found_multiple")));
}
