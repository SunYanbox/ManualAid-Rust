//! Shell tool execution: runs a command through the configured shell and
//! formats stdout, stderr and the exit code into a readable result.
//! Shell 工具执行：通过配置的 Shell 运行命令，并把 stdout、stderr 与退出码
//! 格式化为可读结果。

use std::time::Duration;

use indexmap::IndexMap;
use serde_json::Value;

use super::{ToolResult, get_i64, get_string};
use crate::shell::run_shell;

/// Default and maximum command timeout in milliseconds.
/// 命令超时的默认值与最大值（毫秒）。
const DEFAULT_TIMEOUT_MS: i64 = 120_000;
const MAX_TIMEOUT_MS: i64 = 600_000;

/// Execute one shell command parameter set.
/// 执行一组 shell 命令参数。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let command = match get_string(params, "command") {
        Some(command) => command,
        None => return ToolResult::failure("shell", "Missing required parameter `command`"),
    };

    let timeout_ms = get_i64(params, "timeout")
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1, MAX_TIMEOUT_MS);
    let timeout = Duration::from_millis(timeout_ms as u64);

    match run_shell(&command, Some(timeout)).await {
        Ok(result) => {
            let output = format_output(&result);
            match result.exit_code {
                Some(0) => ToolResult::success("shell", output, false),
                Some(code) => ToolResult::failure(
                    "shell",
                    format!("{output}\nCommand exited with code {code}"),
                ),
                None => ToolResult::failure(
                    "shell",
                    format!("{output}\nCommand was terminated without an exit code"),
                ),
            }
        }
        Err(e) => ToolResult::failure("shell", e.to_string()),
    }
}

/// Format a command result, keeping the exit code visible when non-zero.
/// 格式化命令结果；退出码非零时保持可见。
fn format_output(result: &crate::shell::CommandResult) -> String {
    let mut output = String::new();
    if !result.stdout.is_empty() {
        output.push_str(&result.stdout);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    if !result.stderr.is_empty() {
        output.push_str(&result.stderr);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    if result.timed_out {
        output.push_str("Command timed out");
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_output_combines_streams() {
        let result = crate::shell::CommandResult {
            stdout: "out".into(),
            stderr: "err".into(),
            exit_code: Some(0),
            signal: None,
            timed_out: false,
        };
        assert_eq!(format_output(&result), "out\nerr\n");
    }

    #[test]
    fn format_output_flags_timeout() {
        let result = crate::shell::CommandResult {
            stdout: String::new(),
            stderr: "slow".into(),
            exit_code: None,
            signal: None,
            timed_out: true,
        };
        assert_eq!(format_output(&result), "slow\nCommand timed out\n");
    }
}
