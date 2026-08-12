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

/// Serialize clipboard-touching subprocess tests across processes: each test
/// binary runs its threads concurrently, and the system clipboard is a
/// single shared resource, so a lock file in a unique temp directory keeps
/// one clipboard test running at a time.
/// 跨进程串行化使用剪贴板的子进程测试：每个测试二进制内线程并发运行，而系统
/// 剪贴板是单一共享资源，因此用唯一临时目录中的锁文件保证同一时刻只有一个
/// 剪贴板测试在执行。
struct ClipboardLock;

impl ClipboardLock {
    fn acquire() -> Self {
        let lock_dir = std::env::temp_dir().join(format!(
            "manualaid-cli-clipboard-lock-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&lock_dir).expect("create clipboard lock dir");
        let lock_file = lock_dir.join("lock");
        for _ in 0..200 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_file)
            {
                Ok(_) => return Self,
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
        panic!("could not acquire clipboard lock");
    }
}

impl Drop for ClipboardLock {
    fn drop(&mut self) {
        let lock_dir = std::env::temp_dir().join(format!(
            "manualaid-cli-clipboard-lock-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_file(lock_dir.join("lock"));
    }
}

/// Save the current clipboard text and restore it after `run`, so tests that
/// copy prompts or results do not clobber the user's clipboard. A restore
/// failure is ignored because the clipboard is shared system state.
/// 先保存当前剪贴板文本，`run` 结束后恢复，避免复制提示词或结果的测试覆盖
/// 用户剪贴板。恢复失败被忽略，因为剪贴板是共享的系统状态。
fn with_clipboard_restored(run: impl FnOnce()) {
    let saved = manualaid_core::clipboard::read_clipboard().ok();
    run();
    if let Some(saved) = saved {
        let _ = manualaid_core::clipboard::write_clipboard(saved);
    }
}

/// Poll the system clipboard until it contains `needle` or a timeout
/// elapses. The caller keeps the child binary alive during the poll, so the
/// X11 selection stays owned by the child and no clipboard manager is
/// needed to keep the content readable.
/// 轮询系统剪贴板直到包含 `needle` 或超时。调用方在轮询期间保持子进程
/// 存活，X11 选择权一直由子进程持有，无需剪贴板管理器也能读到内容。
fn wait_for_clipboard_content(needle: &str) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let last = match manualaid_core::clipboard::read_clipboard() {
            Ok(text) if text.contains(needle) => return text,
            Ok(text) => text,
            Err(error) => format!("<clipboard read error: {error}>"),
        };
        assert!(
            std::time::Instant::now() < deadline,
            "clipboard never contained {needle:?}; last read: {last:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

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
    let calls = registry.parse(&read_call).unwrap();
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
fn loop_binary_paste_menu_submits_clipboard_text() {
    let dir = common::TempDir::new("loop-flow-paste");
    let home = common::TempDir::new("loop-flow-paste-home");
    let target = dir.path().join("target.txt");
    std::fs::write(&target, "hello").unwrap();
    let _clipboard_lock = ClipboardLock::acquire();
    with_clipboard_restored(|| {
        manualaid_core::clipboard::write_clipboard(read_call(&target)).expect("set clipboard");
        let output =
            common::run_binary_scripted(dir.path(), Some(home.path()), &[], &["2", "n", "0"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("[read]"));
        assert!(stdout.contains("hello"));
    });
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
    let _clipboard_lock = ClipboardLock::acquire();
    with_clipboard_restored(|| {
        let mut child = common::ScriptedChild::spawn(dir.path(), Some(home.path()), &[]);
        child.send_line("1");
        // Read while the child still owns the X11 selection: the content
        // written by the short-lived binary would be lost once it exits.
        // 在子进程仍持有 X11 选择权时读取：短命二进制退出后内容可能丢失。
        let clipboard = wait_for_clipboard_content("<context_files path=\"AGENTS.md\">");
        assert!(clipboard.contains("<context_files path=\"AGENTS.md\">"));
        assert!(clipboard.contains("# rules"));
        child.send_line("0");
        let output = child.wait_with_output();
        assert!(output.status.success());
    });
}

#[test]
fn loop_binary_asks_selection_when_multiple_context_files_exist() {
    let dir = common::TempDir::new("loop-flow-ctx-multi");
    let home = common::TempDir::new("loop-flow-ctx-multi-home");
    std::fs::write(dir.path().join("AGENTS.md"), "same").unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "same").unwrap();
    let _clipboard_lock = ClipboardLock::acquire();
    with_clipboard_restored(|| {
        let mut child = common::ScriptedChild::spawn(dir.path(), Some(home.path()), &[]);
        child.send_line("1");
        // Pick the first context file from the selection prompt.
        // 在上下文选择提示中选择第一个文件。
        child.send_line("1");
        let clipboard = wait_for_clipboard_content("<context_files path=\"AGENTS.md\">");
        assert!(clipboard.contains("<context_files path=\"AGENTS.md\">"));
        assert!(!clipboard.contains("<context_files path=\"CLAUDE.md\">"));
        child.send_line("0");
        let output = child.wait_with_output();
        assert!(output.status.success());
    });
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
    let _clipboard_lock = ClipboardLock::acquire();
    with_clipboard_restored(|| {
        let mut child = common::ScriptedChild::spawn(dir.path(), Some(home.path()), &[]);
        child.send_line("1");
        // Read while the child still owns the X11 selection: the prompt
        // written by the short-lived binary would be lost once it exits.
        // Poll for this workspace's own path so a stale clipboard from a
        // previous run can never satisfy the wait.
        // 在子进程仍持有 X11 选择权时读取：短命二进制写入的提示词会在它
        // 退出后丢失。轮询本工作区独有的路径，避免上一次运行残留的剪贴板
        // 内容满足等待条件。
        let workspace_marker = dir.path().display().to_string();
        let clipboard = wait_for_clipboard_content(&workspace_marker);
        // The rules text references the <context_files> tag name as a path
        // source, so the assertion targets the rendered block form only.
        // 规则文本会把 <context_files> 标签名作为路径来源引用，因此断言
        // 只针对渲染出的区块形式。
        assert!(!clipboard.contains("<context_files path="));
        child.send_line("0");
        let output = child.wait_with_output();
        assert!(output.status.success());
    });
}
