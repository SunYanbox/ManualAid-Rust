//! Menu action handlers for the interactive loop.
//! 交互式 loop 的菜单动作处理函数。

use std::io::{BufRead, Write};
use std::path::Path;

use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::all_skills;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::LoopOptions;
use super::approval::{ask_approval, execute_round_with_approval};
use super::utils::{format_round_summary, parse_round_index, read_line, t_fmt};

/// Generate the system prompt and copy it to the clipboard.
/// 生成系统提示词并复制到剪贴板。
pub(super) fn copy_system_prompt(config: &Config, root: &Path, registry: &FormatRegistry) {
    let start = std::time::Instant::now();
    let text = manualaid_ws::prompt::build_system_prompt(config, root, registry, &all_skills());
    match manualaid_core::clipboard::write_clipboard(&text) {
        Ok(()) => println!("{}", i18n::t_str("cli.message.prompt_copied")),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
    println!(
        "{}",
        t_fmt(
            "cli.loop.timing_prompt",
            &[("elapsed", &crate::format_duration(start.elapsed()))]
        )
    );
}

/// Read the clipboard and submit its text as one round.
/// 读取剪贴板并把其文本作为一轮提交。
pub(super) async fn paste_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
) {
    let text = match manualaid_core::clipboard::read_clipboard() {
        Ok(text) if text.trim().is_empty() => {
            println!("{}", i18n::t_str("cli.message.clipboard_empty"));
            return;
        }
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}", t_fmt("cli.error.clipboard_read", &[("error", &e)]));
            return;
        }
    };
    submit_text(executor, registry, session, options, &text).await;
}

/// Read multi-line text from stdin (until EOF) and submit it as one round.
/// 从标准输入读取多行文本（直到 EOF）并作为一轮提交。
pub(super) async fn input_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
) {
    println!("{}", i18n::t_str("cli.message.input_prompt"));
    let mut text = String::new();
    for line in std::io::stdin().lock().lines() {
        match line {
            Ok(line) => {
                text.push_str(&line);
                text.push('\n');
            }
            Err(_) => break,
        }
    }
    if text.trim().is_empty() {
        return;
    }
    submit_text(executor, registry, session, options, &text).await;
}

/// Parse and execute input, print the summary and copy results when asked.
/// 解析并执行输入，打印摘要并按需复制结果。
pub(super) async fn submit_text(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    text: &str,
) {
    let round_start = std::time::Instant::now();
    match execute_round_with_approval(executor, registry, text, ask_approval).await {
        Ok((calls, results)) => {
            let _ = crate::pager::print_paged(&format_round_summary(&results));
            session.push(calls, results.clone());
            let round_index = session.len();
            let copy = options.auto_copy || ask_copy();
            if copy
                && let Err(e) = manualaid_core::clipboard::write_clipboard(
                    manualaid_ws::prompt::format_results(&results),
                )
            {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            } else if copy {
                println!(
                    "{}",
                    t_fmt(
                        "cli.message.result_copied",
                        &[("index", &round_index.to_string())]
                    )
                );
            }
            println!(
                "{}",
                t_fmt(
                    "cli.loop.timing_round",
                    &[("elapsed", &crate::format_duration(round_start.elapsed()))]
                )
            );
        }
        Err(e) => eprintln!("{e}"),
    }
}

/// Ask whether to copy the round results to the clipboard.
/// 询问是否将本轮结果复制到剪贴板。
pub(super) fn ask_copy() -> bool {
    print!("{}", i18n::t_str("cli.message.ask_copy"));
    let _ = std::io::stdout().flush();
    read_line().is_some_and(|line| line.trim().eq_ignore_ascii_case("y"))
}

/// Copy the `index`-th latest round (default: latest) to the clipboard.
/// 把从最新算起的第 `index` 轮（默认最新）复制到剪贴板。
pub(super) fn copy_round_result(session: &SessionLog) {
    if session.is_empty() {
        println!("{}", i18n::t_str("cli.message.no_rounds"));
        return;
    }
    println!(
        "{}",
        t_fmt(
            "cli.message.round_count",
            &[("count", &session.len().to_string())]
        )
    );
    print!("{}", i18n::t_str("cli.message.copy_index_prompt"));
    let _ = std::io::stdout().flush();
    let input = read_line().unwrap_or_default();
    match parse_round_index(&input, session.len()) {
        Some(index) => {
            let results = &session.latest(index).expect("validated index").results;
            match manualaid_core::clipboard::write_clipboard(manualaid_ws::prompt::format_results(
                results,
            )) {
                Ok(()) => println!(
                    "{}",
                    t_fmt(
                        "cli.message.result_copied",
                        &[("index", &index.to_string())]
                    )
                ),
                Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
            }
        }
        None => println!(
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
