//! The `debug shell` command: preview, confirm and run one shell command
//! through the real core execution path, printing stdout, stderr, the exit
//! code and the elapsed time.
//! `debug shell` 命令：预览、确认并执行一条 shell 命令，走 core 的真实
//! 执行路径，输出 stdout、stderr、退出码与耗时。

use std::io::Write;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use manualaid_core::audit::{AuditDecision, Auditor};
use manualaid_core::shell::run_shell;
use manualaid_core::tools::ToolKind;
use serde_json::Value;

use super::resolve_arg;
use crate::commands::loop_cli::utils::read_line;
use crate::{format_duration, t_fmt};

/// Default and maximum command timeout in milliseconds, matching the core
/// shell tool.
/// 命令超时的默认值与最大值（毫秒），与 core 的 shell 工具一致。
const DEFAULT_TIMEOUT_MS: i64 = 120_000;
const MAX_TIMEOUT_MS: i64 = 600_000;

/// Preview, confirm and run one shell command, reading the confirmation from
/// the standard input.
/// 预览、确认并执行一条 shell 命令，从标准输入读取确认。
pub async fn run_shell_debug(command: &str, time_out: Option<&str>) -> Result<(), String> {
    run_shell_debug_with_confirm(command, time_out, read_line).await
}

/// Like [`run_shell_debug`] with an injectable confirmation reader, so
/// tests can script the answer without touching the real stdin.
/// 同 [`run_shell_debug`]，但确认读取函数可注入，测试可脚本化答复而不触碰
/// 真实 stdin。
async fn run_shell_debug_with_confirm(
    command: &str,
    time_out: Option<&str>,
    read_confirm: impl FnOnce() -> Option<String>,
) -> Result<(), String> {
    let command = resolve_arg(command)?.trim().to_string();
    if command.is_empty() {
        return Err(t_fmt("cli.debug.shell_command_empty", &[]));
    }
    let timeout_ms = resolve_timeout(time_out)?;

    // 审计先行：黑名单命令直接拒绝，不进入确认与执行阶段。
    let workspace_root = std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))?;
    let auditor = Auditor::new(workspace_root);
    let mut params = IndexMap::new();
    params.insert("command".to_string(), Value::String(command.clone()));
    let decisions = auditor.check(&params, ToolKind::Shell);
    if let Some((_param, AuditDecision::Denied(reason))) = decisions
        .iter()
        .find(|(_, d)| matches!(d, AuditDecision::Denied(_)))
    {
        return Err(t_fmt("cli.debug.shell_denied", &[("reason", reason)]));
    }

    // 预览与确认走 stderr，保持 stdout 只承载命令本身的输出。
    eprintln!(
        "{}",
        t_fmt(
            "cli.debug.shell_preview",
            &[("command", &command), ("timeout", &timeout_ms.to_string()),]
        )
    );
    eprint!("{}", t_fmt("cli.debug.shell_confirm", &[]));
    std::io::stderr()
        .flush()
        .map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))?;
    let answer = read_confirm().unwrap_or_default();
    if !matches!(answer.trim(), "y" | "Y") {
        return Err(t_fmt("cli.debug.shell_aborted", &[]));
    }

    let start = Instant::now();
    let result = run_shell(&command, Some(Duration::from_millis(timeout_ms as u64)))
        .await
        .map_err(|e| e.to_string())?;
    let elapsed = format_duration(start.elapsed());

    if !result.stdout.is_empty() {
        crate::console::out_print!("{}", result.stdout);
    }
    if !result.stderr.is_empty() {
        eprint!("{}", result.stderr);
    }
    let exit_code = result
        .exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "none".to_string());
    crate::console::out_println!(
        "{}",
        t_fmt(
            "cli.debug.shell_result",
            &[
                ("exit_code", &exit_code),
                ("timed_out", &result.timed_out.to_string()),
                ("elapsed", &elapsed),
            ]
        )
    );
    if result.timed_out || result.exit_code != Some(0) {
        return Err(t_fmt("cli.debug.shell_failed", &[]));
    }
    Ok(())
}

/// Parse the optional timeout argument: the default is used when absent,
/// the value is resolved like other content arguments and clamped to the
/// shell tool's valid range.
/// 解析可选超时参数：缺省使用默认值；数值与其他内容参数一样先解析，再
/// 限制在 shell 工具的合法范围内。
fn resolve_timeout(time_out: Option<&str>) -> Result<i64, String> {
    match time_out {
        None => Ok(DEFAULT_TIMEOUT_MS),
        Some(raw) => {
            let value = resolve_arg(raw)?;
            value
                .trim()
                .parse::<i64>()
                .map(|ms| ms.clamp(1, MAX_TIMEOUT_MS))
                .map_err(|e| {
                    t_fmt(
                        "cli.debug.shell_timeout_invalid",
                        &[("error", &e.to_string())],
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{LOCALE_LOCK, temp_dir};

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn aborts_before_executing_when_confirmation_is_no() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("shell-abort");
        let marker = dir.join("marker.txt");
        let command = if cfg!(windows) {
            format!("type nul > {}", marker.display())
        } else {
            format!("touch {}", marker.display())
        };
        let err = run_shell_debug_with_confirm(&command, None, || Some("n".to_string()))
            .await
            .unwrap_err();
        assert!(err.contains("Aborted."));
        assert!(!marker.exists(), "command must not run after abort");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn denied_blacklist_command_never_reaches_confirm_or_execution() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let err = run_shell_debug_with_confirm("rm -rf /", None, || {
            panic!("confirmation must not be requested for a denied command")
        })
        .await
        .unwrap_err();
        assert!(err.contains("denied"), "unexpected error: {err}");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn runs_confirmed_command_and_captures_stdout() {
        let capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let result =
            run_shell_debug_with_confirm("echo hello", None, || Some("y".to_string())).await;
        assert!(result.is_ok(), "unexpected error: {result:?}");
        let text = capture.text();
        assert!(text.contains("hello"), "stdout missing in: {text}");
        assert!(text.contains("exit_code: 0"), "summary missing in: {text}");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn command_writing_to_stderr_still_succeeds() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        // `1>&2` redirects stdout to stderr under both cmd and sh.
        // `1>&2` 在 cmd 与 sh 下都把 stdout 重定向到 stderr。
        let result =
            run_shell_debug_with_confirm("echo err 1>&2", None, || Some("y".to_string())).await;
        assert!(result.is_ok(), "unexpected error: {result:?}");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn rejects_empty_command_before_audit() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let err = run_shell_debug_with_confirm("   ", None, || {
            panic!("confirmation must not be requested for an empty command")
        })
        .await
        .unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn rejects_invalid_timeout() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let err = run_shell_debug_with_confirm("echo hi", Some("abc"), || {
            panic!("confirmation must not be requested for an invalid timeout")
        })
        .await
        .unwrap_err();
        assert!(err.contains("Invalid timeout"));
    }

    #[test]
    fn resolve_timeout_defaults_and_clamps() {
        assert_eq!(resolve_timeout(None).unwrap(), 120_000);
        assert_eq!(resolve_timeout(Some("500")).unwrap(), 500);
        assert_eq!(resolve_timeout(Some("0")).unwrap(), 1);
        assert_eq!(resolve_timeout(Some("999999")).unwrap(), 600_000);
        assert!(resolve_timeout(Some("abc")).is_err());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn non_zero_exit_code_is_a_failure_after_printing_output() {
        let capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let err = run_shell_debug_with_confirm("exit 3", None, || Some("y".to_string()))
            .await
            .unwrap_err();
        assert!(err.contains("failed"), "unexpected error: {err}");
        let text = capture.text();
        assert!(text.contains("exit_code: 3"), "summary missing in: {text}");
    }
}
