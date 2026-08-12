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
use super::utils::{
    format_round_detail, format_round_header, format_round_header_muted, format_round_summary,
    parse_round_index, print_muted_block, read_line, t_fmt,
};

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
        Ok((calls, results, stats)) => {
            let _ = crate::pager::print_paged_collapsed(&format_round_summary(&results));
            session.push(calls, results.clone(), stats);
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
        Some(index) => copy_round_index(session, index, max_result_chars),
        None => crate::console::out_println!(
            "{}",
            t_fmt(
                "cli.error.invalid_index",
                &[("count", &session.len().to_string())]
            )
        ),
    }
}

/// Show the detailed preview of the `index`-th latest round (tools,
/// durations, tokens and the exact content to be copied) and copy its
/// results to the clipboard. The preview is display-only: it is
/// indented, styled down and collapsed-paged so a long round does not
/// flood the console; the clipboard content stays unmodified.
/// 显示从最新算起的第 `index` 轮的详细预览（工具、耗时、Token 与待复制
/// 内容）并把其结果复制到剪贴板。预览仅用于展示：缩进、弱化样式并以
/// 折叠方式分页，避免长轮次刷屏；剪贴板内容不受影响。
/// Maximum content lines shown in the copy preview; the clipboard keeps
/// the full content. Capped so the console never floods even when the
/// pager cannot run (e.g. non-console terminals).
/// 复制预览中显示的最大内容行数；剪贴板保留完整内容。截断保证在分页器
/// 无法工作（如非控制台终端）时控制台也不会被刷屏。
const COPY_PREVIEW_MAX_LINES: usize = 10;

pub(super) fn copy_round_index(session: &SessionLog, index: usize, max_result_chars: usize) {
    let record = session.latest(index).expect("validated index");
    let content = manualaid_ws::prompt::format_results(&record.results, max_result_chars);
    let preview = [
        format_round_header_muted(index, session.len()),
        format_round_detail(record),
        String::new(),
        t_fmt(
            "cli.message.copy_preview",
            &[
                ("index", &index.to_string()),
                ("max_chars", &max_result_chars.to_string()),
            ],
        ),
    ];
    let previewed = truncate_preview_lines(&content, COPY_PREVIEW_MAX_LINES);
    let text = indent_each_line(&(preview.join("\n") + "\n" + &previewed));
    let _ = crate::pager::print_paged_collapsed(&text);
    match manualaid_core::clipboard::write_clipboard(content) {
        Ok(()) => print_muted_block(&[t_fmt(
            "cli.message.result_copied",
            &[("index", &index.to_string())],
        )]),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
}

/// Indent every line of `text` by two spaces for display purposes.
/// 显示用途：给 `text` 的每一行加两个空格的缩进。
fn indent_each_line(text: &str) -> String {
    text.lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cap `text` at `max_lines` lines, appending a note about how many were
/// omitted. Returns the text unchanged when it already fits.
/// 把 `text` 截断到 `max_lines` 行并追加省略说明；未超出时原样返回。
fn truncate_preview_lines(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }
    let omitted = lines.len() - max_lines;
    lines[..max_lines].join("\n")
        + "\n"
        + &t_fmt(
            "cli.message.copy_preview_truncated",
            &[("omitted", &omitted.to_string())],
        )
}

/// Show the recorded rounds, newest first, with per-round tools, timing
/// and token statistics.
/// 展示已记录轮次（最新在前），含每轮工具、耗时与 Token 统计。
pub(super) fn show_tool_history(session: &SessionLog) {
    if session.is_empty() {
        crate::console::out_println!("{}", i18n::t_str("cli.history.empty"));
        return;
    }
    let mut lines = vec![crate::style::header(&i18n::t_str("cli.history.title"))];
    for (i, record) in session.rounds().iter().rev().enumerate() {
        lines.push(format_round_header(i + 1, session.len()));
        lines.push(format_round_detail(record));
        lines.push(String::new());
    }
    let _ = crate::pager::print_paged(&lines.join("\n"));
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
    use manualaid_ws::session::RoundStats;

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
        add_round(root, &mut session).await;
        session
    }

    async fn add_round(root: &Path, session: &mut SessionLog) {
        let registry = FormatRegistry::new();
        let calls = registry.parse(&read_call(root)).unwrap();
        let exec = executor(root);
        let mut results = Vec::new();
        for call in &calls {
            results.push(exec.execute(call.clone()).await);
        }
        session.push(calls, results, RoundStats::default());
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

    #[test]
    fn truncate_preview_lines_keeps_short_text() {
        let text = "a\nb\nc";
        assert_eq!(truncate_preview_lines(text, 10), text);
    }

    #[test]
    fn truncate_preview_lines_caps_long_text() {
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
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
    #[allow(clippy::await_holding_lock)]
    async fn copy_preview_is_indented_and_collapsed() {
        let _capture = crate::console::capture();
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _clip = lock_clipboard();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("copy-preview-indent");
        let session = session_with_round(&root).await;
        let saved = manualaid_core::clipboard::read_clipboard().ok();
        push_test_input(&["1"]);
        copy_round_result(&session, 100);
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
        if let Some(saved) = saved {
            let _ = manualaid_core::clipboard::write_clipboard(saved);
        }
        crate::style::set_enabled(false);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn submit_text_records_round_stats() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("submit-stats");
        let registry = FormatRegistry::new();
        let exec = executor(&root);
        let mut session = SessionLog::new();
        let mut options = LoopOptions {
            auto_copy: false,
            ..LoopOptions::default()
        };
        push_test_input(&["n"]);
        submit_text(
            &exec,
            &registry,
            &mut session,
            &mut options,
            &read_call(&root),
            50000,
        )
        .await;
        let stats = session.latest(1).expect("round recorded").stats;
        assert!(stats.total_tokens > 0);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn show_tool_history_lists_newest_first() {
        let _capture = crate::console::capture();
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("history");
        let mut session = SessionLog::new();
        add_round(&root, &mut session).await;
        add_round(&root, &mut session).await;
        show_tool_history(&session);
        let output = _capture.text();
        let newest = output.find("Round 1 of 2").expect("newest header");
        let oldest = output.find("Round 2 of 2").expect("oldest header");
        assert!(newest < oldest);
        assert!(output.contains("[read]"));
        assert!(output.contains("success"));
        crate::style::set_enabled(false);
    }
}
