//! `@path`-prefixed file/folder references typed in the main menu input
//! box. Files run through the read tool; folders render the directory tree;
//! every existing reference becomes a `[USER_ACTION kind="path"]` result.
//! 在主菜单输入框中输入的以 `@path` 开头的文件/文件夹引用。文件经 read
//! 工具读取；文件夹渲染目录树；每个存在的引用都成为
//! `[USER_ACTION kind="path"]` 结果。

use indexmap::IndexMap;
use serde_json::Value;
use std::path::Path;

use manualaid_core::clipboard::ClipboardProvider;
use manualaid_core::executor::Executor;
use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::{ToolCallFormat, ToolResult, UserActionKind};
use manualaid_ws::config::Config;
use manualaid_ws::session::{RoundStats, SessionLog};

use super::LoopOptions;
use super::handlers::finish_round_with_provider;
use super::utils::t_fmt;
use crate::dir_tree::{DirViewConfig, format_dir_tree};

/// Run every `@token` in `line` as one path round. Returns `true` when at
/// least one `@token` was recognized, even if it does not exist (the
/// localized hint was printed). Returns `false` when no `@token` appears.
/// 把 `line` 中的每个 `@token` 作为一轮路径操作执行。至少识别到一个
/// `@token` 时返回 `true`（即使它不存在，也已打印本地化提示）；没有
/// `@token` 时返回 `false`。
pub(super) async fn run_path_actions<P: ClipboardProvider>(
    provider: &P,
    executor: &Executor,
    root: &Path,
    config: &mut Config,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    line: &str,
) -> bool {
    let mut calls = Vec::new();
    let mut results = Vec::new();
    let mut any_path = false;
    let mut round_start = None;

    for raw_token in line.split_whitespace() {
        if !raw_token.starts_with('@') || raw_token.len() < 2 {
            continue;
        }
        any_path = true;
        // Directory completions may append a trailing separator; strip it
        // before joining so `@dir/` behaves exactly like `@dir`.
        // 目录补全可能追加尾分隔符；在拼接前剥掉，使 `@dir/` 与 `@dir`
        // 行为完全一致。
        let relative = raw_token[1..].trim_end_matches(['/', '\\']);
        let joined = root.join(relative);
        let absolute = std::path::absolute(&joined).unwrap_or_else(|_| joined.clone());

        if joined.is_dir() {
            let output = match format_dir_tree(&absolute, &DirViewConfig::default()) {
                Ok(tree) => tree,
                Err(error) => error.to_string(),
            };
            let result = ToolResult::success("path", output, true)
                .with_user_action(UserActionKind::Path, absolute.display().to_string());
            calls.push(build_path_call(&absolute));
            results.push(result);
            round_start.get_or_insert_with(std::time::Instant::now);
        } else if joined.is_file() {
            let call = build_read_call(&absolute);
            let round_start_local = std::time::Instant::now();
            let mut result = executor.execute(call.clone()).await;
            result.audit_decisions.clear();
            let result =
                result.with_user_action(UserActionKind::Path, absolute.display().to_string());
            calls.push(call);
            results.push(result);
            round_start.get_or_insert(round_start_local);
        } else {
            crate::console::out_println!(
                "{}",
                t_fmt("cli.complete.path_not_found", &[("path", relative)])
            );
        }
    }

    if results.is_empty() {
        return any_path;
    }

    let stats = RoundStats {
        parse_duration_ms: 0,
        audit_duration_ms: 0,
        total_execution_duration_ms: results.iter().map(|r| r.execution_duration_ms).sum(),
        total_tokens: results.iter().map(|r| r.estimated_tokens).sum(),
    };
    finish_round_with_provider(
        provider,
        root,
        session,
        options,
        config.max_result_chars,
        round_start.unwrap_or_else(std::time::Instant::now),
        calls,
        results,
        stats,
    )
    .await;
    true
}

/// Build the synthetic `path` call recorded for a directory reference; the
/// directory is not executed by the executor, but keeping one call per result
/// preserves the session history shape.
/// 为目录引用构建记录用的合成 `path` 调用；目录不经过执行器，但每个结果
/// 对应一个调用可保持会话历史结构。
fn build_path_call(path: &Path) -> ParsedToolCall {
    let mut params = IndexMap::new();
    params.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );

    ParsedToolCall {
        tool_name: "path".to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    }
}

/// Build the `read` call for an existing file reference.
/// 为已存在的文件引用构建 `read` 调用。
fn build_read_call(path: &Path) -> ParsedToolCall {
    let mut params = IndexMap::new();
    params.insert(
        "file_path".to_string(),
        Value::String(path.display().to_string()),
    );

    ParsedToolCall {
        tool_name: "read".to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    }
}

#[cfg(test)]
#[path = "path_action_tests.rs"]
mod tests;
