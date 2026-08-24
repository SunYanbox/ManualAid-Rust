//! `!`-prefixed shell commands typed in the main menu input box.
//! 在主菜单输入框中输入的以 `!` 开头的 Shell 命令。

use indexmap::IndexMap;
use serde_json::Value;

use manualaid_core::clipboard::ClipboardProvider;
use manualaid_core::executor::Executor;
use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::ToolCallFormat;
use manualaid_ws::config::Config;
use manualaid_ws::session::{RoundStats, SessionLog};

use super::LoopOptions;
use super::handlers::finish_round_with_provider;

/// Timeout applied to `!` commands, in milliseconds. Core's shell tool
/// accepts up to 600000 ms; one minute keeps interactive commands from
/// hanging the menu loop for too long.
/// `!` 命令使用的超时时间（毫秒）。core 的 shell 工具最多接受 600000 ms；
/// 一分钟可避免交互命令让菜单循环挂起过久。
const BANG_TIMEOUT_MS: i64 = 60_000;

/// Fixed shell `description` for `!` commands. The user already reviewed
/// the command at the prompt, so no approval description is needed.
/// `!` 命令固定的 shell `description`。用户已在输入框审核过该命令，
/// 因此无需审批描述。
const BANG_DESCRIPTION: &str = "[USER EXEC]";

/// Run a `!`-prefixed shell command as an already-approved round.
/// 把以 `!` 开头的 Shell 命令作为已批准的一轮执行。
pub(super) async fn run_bang_command<P: ClipboardProvider>(
    provider: &P,
    executor: &Executor,
    config: &mut Config,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    line: &str,
) {
    let command = line.trim_start_matches('!').trim();
    if command.is_empty() {
        crate::console::out_println!("{}", i18n::t_str("cli.bang.empty"));
        return;
    }

    let call = build_shell_call(command);
    let round_start = std::time::Instant::now();
    let mut result = executor.execute(call.clone()).await;
    // The user approved the command by typing `!`, so approval hints must
    // not leak into the result summary. Hard denials already failed the
    // result before this point, so clearing decisions only removes
    // cosmetic "needs approval" entries.
    // 用户输入 `!` 即已批准该命令，因此审批提示不能泄漏到结果摘要中。
    // 审计硬拒绝在到达这里之前已让结果失败，清空决策只会移除展示用的
    // “需要批准”条目。
    result.audit_decisions.clear();

    let max_result_chars = config.max_result_chars;
    let stats = RoundStats {
        parse_duration_ms: 0,
        audit_duration_ms: 0,
        total_execution_duration_ms: result.execution_duration_ms,
        total_tokens: result.estimated_tokens,
    };
    let calls = vec![call];
    let results = vec![result];
    finish_round_with_provider(
        provider,
        session,
        options,
        max_result_chars,
        round_start,
        calls,
        results,
        stats,
    )
    .await;
}

/// Build the shell `ParsedToolCall` for a `!` command.
/// 为 `!` 命令构建 shell `ParsedToolCall`。
fn build_shell_call(command: &str) -> ParsedToolCall {
    let mut params = IndexMap::new();
    params.insert("command".to_string(), Value::String(command.to_string()));
    params.insert(
        "description".to_string(),
        Value::String(BANG_DESCRIPTION.to_string()),
    );
    params.insert("timeout".to_string(), Value::Number(BANG_TIMEOUT_MS.into()));

    ParsedToolCall {
        tool_name: "shell".to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    }
}

#[cfg(test)]
#[path = "bang_tests.rs"]
mod tests;
