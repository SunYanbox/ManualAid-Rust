//! Menu action handlers for the interactive loop.
//! 交互式 loop 的菜单动作处理函数。

use std::path::Path;

use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::all_skills;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::LoopOptions;
use super::approval::{ask_approval, execute_round_with_approval};
use super::context::select_context_files;
use super::utils::{format_round_summary, parse_round_index, print_muted_block, read_line, t_fmt};

/// Generate the system prompt with the selected context files and copy it
/// to the clipboard. Context files are resolved at this point so the
/// selection question is only asked when the prompt is actually generated.
/// 结合所选上下文文件生成系统提示词并复制到剪贴板。上下文文件在此刻解析，因此只在真正
/// 生成提示词时才询问选择。
pub(super) fn copy_system_prompt(config: &Config, root: &Path, registry: &FormatRegistry) {
    let start = std::time::Instant::now();
    let context_files = if config.context_auto_load {
        select_context_files(root)
    } else {
        Vec::new()
    };
    let text = manualaid_ws::prompt::build_system_prompt(
        config,
        root,
        registry,
        &all_skills(),
        &context_files,
    );
    let mut block = vec![t_fmt(
        "cli.loop.timing_prompt",
        &[("elapsed", &crate::format_duration(start.elapsed()))],
    )];
    match manualaid_core::clipboard::write_clipboard(&text) {
        Ok(()) => block.insert(0, i18n::t_str("cli.message.prompt_copied")),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
    print_muted_block(&block);
}

/// Read the clipboard and submit its text as one round.
/// 读取剪贴板并把其文本作为一轮提交。
pub(super) async fn paste_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    max_result_chars: usize,
) {
    let text = match manualaid_core::clipboard::read_clipboard() {
        Ok(text) if text.trim().is_empty() => {
            crate::console::out_println!("{}", i18n::t_str("cli.message.clipboard_empty"));
            return;
        }
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}", t_fmt("cli.error.clipboard_read", &[("error", &e)]));
            return;
        }
    };
    submit_text(
        executor,
        registry,
        session,
        options,
        &text,
        max_result_chars,
    )
    .await;
}

/// The line that ends a manually typed round; EOF still works as a
/// fallback terminator for piped input.
/// 结束手动输入一轮的行标记；EOF 仍可作为管道输入的兜底结束方式。
const INPUT_END_MARKER: &str = "/end";

/// Read multi-line text from stdin until a lone `/end` line (or EOF) and
/// submit it as one round. EOF keeps working for piped input, while the
/// marker lets an interactive session continue after the round.
/// 从标准输入读取多行文本，直到单独的 `/end` 行（或 EOF）并作为一轮
/// 提交。EOF 仍支持管道输入，标记行则让交互会话在一轮后可以继续。
pub(super) async fn input_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    max_result_chars: usize,
) {
    crate::console::out_println!("{}", i18n::t_str("cli.message.input_prompt"));
    let mut text = String::new();
    while let Some(line) = read_line() {
        if line.trim() == INPUT_END_MARKER {
            break;
        }
        text.push_str(&line);
        text.push('\n');
    }
    if text.trim().is_empty() {
        return;
    }
    submit_text(
        executor,
        registry,
        session,
        options,
        &text,
        max_result_chars,
    )
    .await;
}

/// Parse and execute input, print the summary and copy results when asked.
/// 解析并执行输入，打印摘要并按需复制结果。
pub(super) async fn submit_text(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    text: &str,
    max_result_chars: usize,
) {
    let round_start = std::time::Instant::now();
    match execute_round_with_approval(executor, registry, text, ask_approval).await {
        Ok((calls, results)) => {
            let _ = crate::pager::print_paged_collapsed(&format_round_summary(&results));
            session.push(calls, results.clone());
            let round_index = session.len();
            let copy = options.auto_copy || ask_copy();
            let mut block = vec![t_fmt(
                "cli.loop.timing_round",
                &[("elapsed", &crate::format_duration(round_start.elapsed()))],
            )];
            if copy
                && let Err(e) = manualaid_core::clipboard::write_clipboard(
                    manualaid_ws::prompt::format_results(&results, max_result_chars),
                )
            {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            } else if copy {
                block.insert(
                    0,
                    t_fmt(
                        "cli.message.result_copied",
                        &[("index", &round_index.to_string())],
                    ),
                );
            }
            print_muted_block(&block);
        }
        Err(e) => eprintln!("{e}"),
    }
}

/// Ask whether to copy the round results to the clipboard.
/// 询问是否将本轮结果复制到剪贴板。
pub(super) fn ask_copy() -> bool {
    crate::console::out_print!("{}", i18n::t_str("cli.message.ask_copy"));
    crate::console::flush();
    read_line().is_some_and(|line| line.trim().eq_ignore_ascii_case("y"))
}

/// Copy the `index`-th latest round (default: latest) to the clipboard.
/// 把从最新算起的第 `index` 轮（默认最新）复制到剪贴板。
pub(super) fn copy_round_result(session: &SessionLog, max_result_chars: usize) {
    if session.is_empty() {
        crate::console::out_println!("{}", i18n::t_str("cli.message.no_rounds"));
        return;
    }
    crate::console::out_println!(
        "{}",
        t_fmt(
            "cli.message.round_count",
            &[("count", &session.len().to_string())]
        )
    );
    crate::console::out_print!("{}", i18n::t_str("cli.message.copy_index_prompt"));
    crate::console::flush();
    let input = read_line().unwrap_or_default();
    match parse_round_index(&input, session.len()) {
        Some(index) => {
            let results = &session.latest(index).expect("validated index").results;
            match manualaid_core::clipboard::write_clipboard(manualaid_ws::prompt::format_results(
                results,
                max_result_chars,
            )) {
                Ok(()) => print_muted_block(&[t_fmt(
                    "cli.message.result_copied",
                    &[("index", &index.to_string())],
                )]),
                Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
            }
        }
        None => crate::console::out_println!(
            "{}",
            t_fmt(
                "cli.error.invalid_index",
                &[("count", &session.len().to_string())]
            )
        ),
    }
}

/// Print the session summary (round count, tool-call count, enabled tools).
/// 打印会话摘要（批次数量、工具调用数量、已启用工具）。
pub(super) fn print_session_summary(config: &Config, session: &SessionLog) {
    let tools = config.enabled_tool_names().join(", ");
    let text = [
        crate::style::header(&i18n::t_str("cli.message.summary_title")),
        t_fmt(
            "cli.message.summary_rounds",
            &[("count", &session.len().to_string())],
        ),
        t_fmt(
            "cli.message.summary_tool_calls",
            &[("count", &session.total_calls().to_string())],
        ),
        t_fmt("cli.message.summary_enabled_tools", &[("tools", &tools)]),
    ]
    .join("\n");
    let _ = crate::pager::print_paged(&text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use super::super::utils::push_test_input;
    use manualaid_core::audit::{Auditor, SessionMode};

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
        let registry = FormatRegistry::new();
        let calls = registry.parse(&read_call(root)).unwrap();
        let exec = executor(root);
        let mut results = Vec::new();
        for call in &calls {
            results.push(exec.execute(call.clone()).await);
        }
        session.push(calls, results);
        session
    }

    #[test]
    fn ask_copy_accepts_yes_ignores_rest() {
        let _capture = crate::console::capture();
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
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("summary");
        let session = session_with_round(&root).await;
        print_session_summary(&Config::default(), &session);
    }

    #[test]
    fn copy_round_result_without_rounds_prints_notice() {
        let _capture = crate::console::capture();
        let session = SessionLog::new();
        copy_round_result(&session, 100);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn copy_round_result_rejects_out_of_range_index() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("copy-index");
        let session = session_with_round(&root).await;
        push_test_input(&["9"]);
        copy_round_result(&session, 100);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn input_and_submit_eof_without_text_is_noop() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("input-eof");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        push_test_input(&[]);
        input_and_submit(
            &executor(&root),
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
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("input-marker");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        push_test_input(&["/end"]);
        input_and_submit(
            &executor(&root),
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
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("input-round");
        let mut session = SessionLog::new();
        let mut options = LoopOptions {
            auto_copy: false,
            ..LoopOptions::default()
        };
        push_test_input(&[&read_call(&root), "/end", "n"]);
        input_and_submit(
            &executor(&root),
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
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("submit-parse");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        submit_text(
            &executor(&root),
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
    // The lock must span the await so no concurrent test touches the
    // clipboard while this round reads or writes it.
    // 锁须跨 await 持有，避免并发测试在本轮读写剪贴板时访问剪贴板。
    #[allow(clippy::await_holding_lock)]
    async fn submit_text_with_auto_copy_asks_and_skips_copy_on_no() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("submit-autocopy");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        push_test_input(&["n"]);
        submit_text(
            &executor(&root),
            &FormatRegistry::new(),
            &mut session,
            &mut options,
            &read_call(&root),
            100,
        )
        .await;
        assert_eq!(session.len(), 1);
    }

    // The clipboard tests below save the user's clipboard text first and
    // restore it afterwards; the in-process lock serializes them against
    // concurrent clipboard access within this process.
    // 以下剪贴板测试先保存用户剪贴板文本，结束后恢复；进程内锁保证与同进程
    // 的并发剪贴板访问串行。
    fn lock_clipboard() -> std::sync::MutexGuard<'static, ()> {
        // A failed clipboard test must not poison the shared lock for the
        // remaining tests, so a poisoned guard is recovered like the shell
        // tests do.
        // 某个剪贴板测试失败时不能毒化共享锁拖垮其余测试，因此与 shell
        // 测试一样对毒化守卫做恢复。
        crate::test_support::CLIPBOARD_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with_clipboard_restored(run: impl FnOnce()) {
        let saved = manualaid_core::clipboard::read_clipboard().ok();
        run();
        if let Some(saved) = saved {
            let _ = manualaid_core::clipboard::write_clipboard(saved);
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn paste_and_submit_pastes_clipboard_text_as_a_round() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("paste-round");
        let saved = manualaid_core::clipboard::read_clipboard().ok();
        manualaid_core::clipboard::write_clipboard(read_call(&root))
            .expect("set clipboard for pasting");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        push_test_input(&["n"]);
        paste_and_submit(
            &executor(&root),
            &FormatRegistry::new(),
            &mut session,
            &mut options,
            100,
        )
        .await;
        assert_eq!(session.len(), 1);
        if let Some(saved) = saved {
            let _ = manualaid_core::clipboard::write_clipboard(saved);
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn paste_and_submit_with_empty_clipboard_is_noop() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("paste-empty");
        let saved = manualaid_core::clipboard::read_clipboard().ok();
        manualaid_core::clipboard::write_clipboard("").expect("clear clipboard");
        let mut session = SessionLog::new();
        let mut options = LoopOptions::default();
        paste_and_submit(
            &executor(&root),
            &FormatRegistry::new(),
            &mut session,
            &mut options,
            100,
        )
        .await;
        assert_eq!(session.len(), 0);
        if let Some(saved) = saved {
            let _ = manualaid_core::clipboard::write_clipboard(saved);
        }
    }

    #[test]
    fn copy_system_prompt_writes_prompt_to_clipboard() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("copy-prompt");
        with_clipboard_restored(|| {
            copy_system_prompt(&Config::default(), &root, &FormatRegistry::new());
            let clipboard = manualaid_core::clipboard::read_clipboard().expect("read clipboard");
            assert!(clipboard.contains("<read>"));
        });
    }

    #[test]
    fn copy_system_prompt_includes_selected_context_files() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("copy-prompt-context");
        std::fs::write(root.join("AGENTS.md"), "# project rules").unwrap();
        with_clipboard_restored(|| {
            copy_system_prompt(&Config::default(), &root, &FormatRegistry::new());
            let clipboard = manualaid_core::clipboard::read_clipboard().expect("read clipboard");
            let dynamic = clipboard
                .split_once("<dynamic-context>")
                .and_then(|(_, rest)| rest.split_once("</dynamic-context>"))
                .map(|(inner, _)| inner)
                .unwrap_or_default();
            assert!(dynamic.contains("<context_files path=\"AGENTS.md\">"));
            assert!(dynamic.contains("# project rules"));
        });
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn copy_round_result_copies_selected_round() {
        let _capture = crate::console::capture();
        let _lock = lock_clipboard();
        let root = crate::test_support::temp_dir("copy-valid");
        let session = session_with_round(&root).await;
        let saved = manualaid_core::clipboard::read_clipboard().ok();
        push_test_input(&["1"]);
        copy_round_result(&session, 100);
        let clipboard = manualaid_core::clipboard::read_clipboard().expect("read clipboard");
        assert!(clipboard.contains("hello"));
        if let Some(saved) = saved {
            let _ = manualaid_core::clipboard::write_clipboard(saved);
        }
    }
}
