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
use manualaid_core::clipboard::{ClipboardProvider, RealClipboard};
use tokenx_rs;

/// Generate the system prompt with the selected context files and copy it
/// to the clipboard. Context files are resolved at this point so the
/// selection question is only asked when the prompt is actually generated.
/// 结合所选上下文文件生成系统提示词并复制到剪贴板。上下文文件在此刻解析，因此只在真正
/// 生成提示词时才询问选择。
pub fn copy_system_prompt_with_provider<P: ClipboardProvider>(
    provider: &P,
    config: &Config,
    root: &Path,
    registry: &FormatRegistry,
) {
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
    let tokens = tokenx_rs::estimate_token_count(&text);
    let mut block = vec![
        t_fmt(
            "cli.loop.timing_prompt",
            &[("elapsed", &crate::format_duration(start.elapsed()))],
        ),
        t_fmt(
            "cli.loop.token_estimate_prompt",
            &[("tokens", &tokens.to_string())],
        ),
    ];
    match provider.write(&text) {
        Ok(()) => block.insert(0, i18n::t_str("cli.message.prompt_copied")),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
    print_muted_block(&block);
}

/// Copy the intent-output-rule text to the clipboard so the user can
/// paste it into an external LLM chat to re-emphasise the rule mid-session.
/// 将意图规则文本复制到剪贴板，方便用户在外部 LLM 聊天中途重新强调该规则。
pub fn copy_intent_rule_with_provider<P: ClipboardProvider>(provider: &P) {
    let text = i18n::t_str("prompt.system.intent-output-rule");
    match provider.write(&text) {
        Ok(()) => print_muted_block(&[i18n::t_str("cli.message.intent_rule_copied")]),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
}

/// Read the clipboard and submit its text as one round.
/// 读取剪贴板并把其文本作为一轮提交。
pub async fn paste_and_submit_with_provider<P: ClipboardProvider>(
    provider: &P,
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    max_result_chars: usize,
) {
    let text = match provider.read() {
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
    submit_text_with_provider(
        provider,
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
pub async fn input_and_submit(
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
pub async fn submit_text(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    text: &str,
    max_result_chars: usize,
) {
    submit_text_with_provider(
        &RealClipboard,
        executor,
        registry,
        session,
        options,
        text,
        max_result_chars,
    )
    .await;
}

pub async fn submit_text_with_provider<P: ClipboardProvider>(
    provider: &P,
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
            let round_tokens = stats.total_tokens;
            session.push(calls, results.clone(), stats);
            let round_index = session.len();
            let copy = options.auto_copy || ask_copy();
            let mut block = vec![
                t_fmt(
                    "cli.loop.timing_round",
                    &[("elapsed", &crate::format_duration(round_start.elapsed()))],
                ),
                t_fmt(
                    "cli.loop.token_estimate_round",
                    &[("tokens", &round_tokens.to_string())],
                ),
            ];
            if copy
                && let Err(e) = provider.write(&manualaid_ws::prompt::format_results(
                    &results,
                    max_result_chars,
                ))
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
pub fn ask_copy() -> bool {
    crate::console::out_print!("{}", i18n::t_str("cli.message.ask_copy"));
    crate::console::flush();
    read_line().is_some_and(|line| line.trim().eq_ignore_ascii_case("y"))
}

/// Copy the `index`-th latest round (default: latest) to the clipboard.
/// 把从最新算起的第 `index` 轮（默认最新）复制到剪贴板。
pub fn copy_round_result(session: &SessionLog, max_result_chars: usize) {
    copy_round_result_with_provider(&RealClipboard, session, max_result_chars);
}

pub fn copy_round_result_with_provider<P: ClipboardProvider>(
    provider: &P,
    session: &SessionLog,
    max_result_chars: usize,
) {
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
        Some(index) => copy_round_index_with_provider(provider, session, index, max_result_chars),
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

pub(super) fn copy_round_index_with_provider<P: ClipboardProvider>(
    provider: &P,
    session: &SessionLog,
    index: usize,
    max_result_chars: usize,
) {
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
    // The content preview is display-only: faint gray so the copyable
    // details stand out and the content stays in the background.
    // 内容预览仅用于展示：用浅灰样式，让详情信息突出、内容退居背景。
    let previewed = crate::style::gray(&truncate_preview_lines(&content, COPY_PREVIEW_MAX_LINES));
    let text = indent_each_line(&(preview.join("\n") + "\n" + &previewed));
    let _ = crate::pager::print_paged_collapsed(&text);
    match provider.write(&content) {
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
pub fn truncate_preview_lines(text: &str, max_lines: usize) -> String {
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
/// and token statistics, plus the session totals in the title line.
/// 展示已记录轮次（最新在前），含每轮工具、耗时与 Token 统计，并在标题
/// 行显示会话总计。
pub fn show_tool_history(session: &SessionLog) {
    if session.is_empty() {
        crate::console::out_println!("{}", i18n::t_str("cli.history.empty"));
        return;
    }
    let tokens: u64 = session
        .rounds()
        .iter()
        .map(|round| round.stats.total_tokens)
        .sum();
    let duration_ms: u64 = session
        .rounds()
        .iter()
        .map(|round| {
            round.stats.parse_duration_ms
                + round.stats.audit_duration_ms
                + round.stats.total_execution_duration_ms
        })
        .sum();
    let totals = t_fmt(
        "cli.history.totals",
        &[
            ("tokens", &tokens.to_string()),
            (
                "duration",
                &crate::format_duration(std::time::Duration::from_millis(duration_ms)),
            ),
        ],
    );
    let mut lines = vec![crate::style::header(&format!(
        "{}  {}",
        i18n::t_str("cli.history.title"),
        totals
    ))];
    for (i, record) in session.rounds().iter().rev().enumerate() {
        lines.push(format_round_header(i + 1, session.len()));
        lines.push(format_round_detail(record));
        lines.push(String::new());
    }
    let _ = crate::pager::print_paged(&lines.join("\n"));
}

/// Print the session summary (round count, tool-call count, enabled tools).
/// 打印会话摘要（批次数量、工具调用数量、已启用工具）。
pub fn print_session_summary(config: &Config, session: &SessionLog) {
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
