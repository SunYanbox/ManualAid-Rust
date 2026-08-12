//! Small utilities for the loop: formatting, input and cycling helpers.
//! loop 的小工具函数：格式化、输入与循环切换辅助。

use std::io::IsTerminal;
use std::path::Path;

use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::tools::ToolResult;
use manualaid_ws::config::{Config, ConfigIssue, ConfigIssueKind};
use manualaid_ws::session::BatchRecord;

/// Translate `key` and replace `%{name}` placeholders.
/// 翻译 `key` 并替换 `%{name}` 占位符。
pub(super) fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
}

/// Format one config-file validation issue as a user-facing warning line.
/// 把一条配置校验问题格式化为面向用户的警告行。
pub(super) fn format_config_issue(issue: &ConfigIssue) -> String {
    match issue.kind {
        ConfigIssueKind::InvalidValue => t_fmt(
            "cli.warning.invalid_config_value",
            &[
                ("key", &issue.key),
                ("value", &issue.value),
                ("available", &issue.available_values.join(", ")),
                ("path", &issue.path.display().to_string()),
            ],
        ),
        ConfigIssueKind::DangerousAllowCommand => t_fmt(
            "cli.warning.dangerous_allow_command",
            &[
                ("command", &issue.value),
                ("path", &issue.path.display().to_string()),
            ],
        ),
    }
}

/// Read one line from stdin; EOF or an error yields `None`.
/// 从标准输入读取一行；EOF 或出错返回 `None`。
pub(super) fn read_line() -> Option<String> {
    #[cfg(test)]
    {
        // Unit tests drive interactive loops with a scripted per-thread
        // input queue; an empty queue acts as EOF so the real stdin is
        // never read (and never blocks) in tests.
        // 单元测试用线程本地的脚本输入队列驱动交互循环；队列为空视为
        // EOF，测试中绝不读取（也不会阻塞）真实 stdin。
        TEST_INPUT.with(|input| input.borrow_mut().pop_front())
    }
    #[cfg(not(test))]
    {
        use std::io::BufRead;
        let mut line = String::new();
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(line),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
thread_local! {
    static TEST_INPUT: std::cell::RefCell<std::collections::VecDeque<String>> =
        const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

/// Queue scripted input lines for the current test thread.
/// 为当前测试线程排队脚本输入行。
#[cfg(test)]
pub(super) fn push_test_input(lines: &[&str]) {
    TEST_INPUT.with(|input| {
        let mut queue = input.borrow_mut();
        for line in lines {
            queue.push_back((*line).to_string());
        }
    });
}

/// Clear the screen via the platform command; failures are ignored. The
/// command never runs while a test capture is active or when stdout is not
/// a real terminal, so `cargo test` can never clear the user's console.
/// 通过平台命令清屏；失败时静默忽略。测试捕获期间或 stdout 非真实终端时
/// 绝不执行命令，`cargo test` 因此永远不会清掉用户的控制台。
pub(super) fn clear_screen() {
    // Test builds link the test crate, where the real terminal is visible
    // to the process; never run the platform clear command there, so
    // `cargo test` cannot clear the user's console even without a capture.
    // 测试构建链接的是 test crate，真实终端对进程可见；此时绝不执行平台
    // 清屏命令，`cargo test` 即使没有捕获守卫也不会清掉用户的控制台。
    if cfg!(test) {
        return;
    }
    if crate::console::is_capturing() || !std::io::stdout().is_terminal() {
        return;
    }
    let _ = clear_command_status(
        std::process::Stdio::inherit(),
        std::process::Stdio::inherit(),
    );
}

/// Run the platform clear command and return its status; failures are
/// surfaced so callers can decide whether clearing actually happened.
/// 运行平台清屏命令并返回其状态；失败时由调用方决定如何处理。
#[cfg(target_os = "windows")]
fn clear_command_status(
    stdin: std::process::Stdio,
    stdout: std::process::Stdio,
) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("cmd")
        .args(["/C", "cls"])
        .stdin(stdin)
        .stdout(stdout)
        .stderr(std::process::Stdio::null())
        .status()
}

#[cfg(not(target_os = "windows"))]
fn clear_command_status(
    stdin: std::process::Stdio,
    stdout: std::process::Stdio,
) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("clear")
        .stdin(stdin)
        .stdout(stdout)
        .stderr(std::process::Stdio::null())
        .status()
}

/// Apply an explicit `-l/--lang` override to the session config. Invalid
/// values are ignored so a typo never breaks the loop startup.
/// 把显式传入的 `-l/--lang` 覆盖到会话配置。非法值会被忽略，避免拼写
/// 错误导致 loop 无法启动。
pub(super) fn apply_cli_lang(cli_lang: Option<String>, config: &mut Config) {
    if let Some(lang) = cli_lang.filter(|lang| Config::is_valid_lang(lang)) {
        config.lang = lang;
    }
}

/// Persist `max_result_chars` to the project config file so the effective
/// limit is always visible and editable there, and return one message per
/// `[global]` value that differs from its default. A failed write yields the
/// error message instead; it never aborts the loop.
/// 把 `max_result_chars` 持久化到项目配置文件，使生效的限额始终在该文件
/// 中可见可改；`[global]` 下每个不同于默认值的配置各返回一条消息。写入
/// 失败时只返回错误消息；写入失败不会中止 loop。
pub fn sync_global_config(root: &Path, config: &Config) -> Vec<String> {
    let defaults = Config::default();
    if let Err(e) = manualaid_ws::config::save_max_result_chars(root, config.max_result_chars) {
        return vec![t_fmt(
            "cli.error.config_write",
            &[("error", &e.to_string())],
        )];
    }
    let path = root
        .join(".ManualAid")
        .join("config.toml")
        .display()
        .to_string();
    let mut messages = Vec::new();
    if config.lang != defaults.lang {
        messages.push(changed_global_hint(
            &path,
            "lang",
            &format!("\"{}\"", config.lang),
            &format!("\"{}\"", defaults.lang),
        ));
    }
    if config.tool_call_format != defaults.tool_call_format {
        messages.push(changed_global_hint(
            &path,
            "tool_call_format",
            &format!("\"{}\"", config.tool_call_format),
            &format!("\"{}\"", defaults.tool_call_format),
        ));
    }
    if config.max_result_chars != defaults.max_result_chars {
        messages.push(changed_global_hint(
            &path,
            "max_result_chars",
            &config.max_result_chars.to_string(),
            &defaults.max_result_chars.to_string(),
        ));
    }
    if config.context_auto_load != defaults.context_auto_load {
        messages.push(changed_global_hint(
            &path,
            "context_auto_load",
            &config.context_auto_load.to_string(),
            &defaults.context_auto_load.to_string(),
        ));
    }
    messages
}

/// Render one "config file changed the default" hint; string values are
/// quoted so the line reads like TOML.
/// 渲染一条「配置文件更改了默认值」的提示；字符串值加引号，使该行与
/// TOML 写法一致。
fn changed_global_hint(path: &str, key: &str, value: &str, default: &str) -> String {
    t_fmt(
        "cli.message.global_config_changed",
        &[
            ("path", path),
            ("key", key),
            ("value", value),
            ("default", default),
        ],
    )
}

/// Apply the configured format label to the registry.
/// 将配置的格式标签应用到注册表。
pub(super) fn apply_format_mode(registry: &FormatRegistry, config: &Config) -> Result<(), String> {
    let mode = RegistryMode::from_label(&config.tool_call_format)
        .ok_or_else(|| format!("Unknown format label `{}`", config.tool_call_format))?;
    registry.set_mode(mode).map_err(|e| e.to_string())
}

/// Render the main menu text.
/// 渲染主菜单文本。
pub fn render_menu() -> String {
    let keys = [
        "cli.loop.menu_title",
        "cli.loop.menu_generate",
        "cli.loop.menu_paste",
        "cli.loop.menu_input",
        "cli.loop.menu_copy",
        "cli.loop.menu_config",
        "cli.loop.menu_summary",
        "cli.loop.menu_history",
        "cli.loop.menu_exit",
    ];
    let mut lines: Vec<String> = keys.iter().map(|key| i18n::t_str(key)).collect();
    lines[0] = crate::style::header(&lines[0]);
    lines.join("\n") + "\n"
}

/// Cycle the interface language between `en` and `zh-CN`.
/// 在 `en` 与 `zh-CN` 之间循环切换界面语言。
pub fn cycle_lang(current: &str) -> String {
    if current == "en" {
        "zh-CN".to_string()
    } else {
        "en".to_string()
    }
}

/// Cycle the tool-call format through `auto` → `xml` → `json-codeblock`.
/// 按 `auto` → `xml` → `json-codeblock` 循环切换工具调用格式。
pub fn cycle_format(current: &str) -> String {
    let labels = RegistryMode::all_labels();
    let index = labels
        .iter()
        .position(|label| *label == current)
        .unwrap_or(0);
    labels[(index + 1) % labels.len()].to_string()
}

/// Parse a round index input (`1` = latest). Empty input means `1`; an
/// out-of-range or non-numeric value yields `None`.
/// 解析批次索引输入（`1` = 最新）。空输入表示 `1`；越界或非数字返回
/// `None`。
pub fn parse_round_index(input: &str, total: usize) -> Option<usize> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(1);
    }
    let index: usize = trimmed.parse().ok()?;
    (1..=total).contains(&index).then_some(index)
}

/// Render the console summary of one round's results.
/// 渲染一轮执行结果的控制台摘要。
pub fn format_round_summary(results: &[ToolResult]) -> String {
    let mut out = String::new();
    for result in results {
        let state = if result.success {
            crate::style::success(&i18n::t_str("cli.message.success"))
        } else {
            crate::style::error(&i18n::t_str("cli.message.failure"))
        };
        let tool = crate::style::accent(&format!("[{}]", result.tool_name));
        out.push_str(&format!("{tool} {state}\n"));
        // Iterate the raw output so no leading or trailing whitespace of a
        // `read` slice is dropped from the console summary.
        // 按原文逐行输出，避免 `read` 切片的首尾空白在控制台摘要中丢失。
        for line in result.output.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        for (param, decision) in &result.audit_decisions {
            if let Some(reason) = decision.reason() {
                out.push_str(&crate::style::yellow(&format!(
                    "  {}: {param} ({reason})\n",
                    i18n::t_str("cli.message.approval_needed")
                )));
            }
        }
        out.push('\n');
    }
    out
}

/// Print `lines` as a muted block with one blank line above and below, so
/// short status messages stand apart from the surrounding output without
/// drawing attention.
/// 把 `lines` 以弱化样式打印为上下各带一个空行的区块，让短状态消息与
/// 周边输出分开，同时不喧宾夺主。
/// Render the header line of one round (its index and the total count).
/// 渲染一轮的标题行（序号与总轮数）。
pub fn format_round_header(index: usize, total: usize) -> String {
    crate::style::header(&t_fmt(
        "cli.history.round",
        &[("index", &index.to_string()), ("count", &total.to_string())],
    ))
}

/// Render one round's detail: a line per tool with its status, execution
/// duration and token estimate, followed by a footer with the parse/audit/
/// execution durations and the round token total. Shared by the history
/// list and the copy preview.
/// 渲染一轮的详情：每个工具一行（状态、执行耗时与 Token 估算），末尾
/// 一行显示解析/审批/执行耗时与轮 Token 总量。历史列表与复制预览共用。
pub fn format_round_detail(record: &BatchRecord) -> String {
    let mut lines: Vec<String> = record
        .results
        .iter()
        .map(|result| {
            let status = if result.success {
                crate::style::success(&i18n::t_str("cli.message.success"))
            } else {
                crate::style::error(&i18n::t_str("cli.message.failure"))
            };
            t_fmt(
                "cli.history.tool_line",
                &[
                    (
                        "tool",
                        &crate::style::accent(&format!("[{}]", result.tool_name)),
                    ),
                    ("status", &status),
                    (
                        "duration",
                        &crate::format_duration(std::time::Duration::from_millis(
                            result.execution_duration_ms,
                        )),
                    ),
                    ("tokens", &result.estimated_tokens.to_string()),
                ],
            )
        })
        .collect();
    let stats = &record.stats;
    lines.push(crate::style::muted(&t_fmt(
        "cli.history.timing_line",
        &[
            (
                "parse",
                &crate::format_duration(std::time::Duration::from_millis(stats.parse_duration_ms)),
            ),
            (
                "audit",
                &crate::format_duration(std::time::Duration::from_millis(stats.audit_duration_ms)),
            ),
            (
                "execution",
                &crate::format_duration(std::time::Duration::from_millis(
                    stats.total_execution_duration_ms,
                )),
            ),
            ("tokens", &stats.total_tokens.to_string()),
        ],
    )));
    lines.join("\n")
}

pub(super) fn print_muted_block(lines: &[String]) {
    crate::console::out_println!();
    for line in lines {
        crate::console::out_println!("{}", crate::style::muted(line));
    }
    crate::console::out_println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use manualaid_ws::session::RoundStats;

    fn record_with_stats() -> BatchRecord {
        BatchRecord {
            calls: Vec::new(),
            results: vec![
                ToolResult::success("read", "hello", true),
                ToolResult::failure("edit", "boom"),
            ],
            stats: RoundStats {
                parse_duration_ms: 12,
                audit_duration_ms: 34,
                total_execution_duration_ms: 56,
                total_tokens: 789,
            },
        }
    }

    #[test]
    fn format_round_detail_shows_tools_and_totals() {
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let detail = format_round_detail(&record_with_stats());
        assert!(detail.contains("[read]"));
        assert!(detail.contains("success"));
        assert!(detail.contains("[edit]"));
        assert!(detail.contains("failure"));
        assert!(detail.contains("parse"));
        assert!(detail.contains("audit"));
        assert!(detail.contains("exec"));
        assert!(detail.contains("789"));
        crate::style::set_enabled(false);
    }

    #[test]
    fn read_line_consumes_scripted_input_then_eof() {
        push_test_input(&["first", "second"]);
        assert_eq!(read_line().as_deref(), Some("first"));
        assert_eq!(read_line().as_deref(), Some("second"));
        assert_eq!(read_line(), None);
    }

    #[test]
    fn clear_screen_skips_while_capturing() {
        // With a capture active the platform clear command never runs, so
        // the user's console stays untouched no matter how cargo test is
        // launched.
        // 捕获状态下平台清屏命令绝不执行，无论 cargo test 如何启动，
        // 用户的控制台都不会被清空。
        let capture = crate::console::capture();
        clear_screen();
        assert_eq!(capture.text(), "");
    }

    #[test]
    fn clear_screen_is_a_noop_in_test_builds() {
        // Without a capture guard the command must still never run: unit
        // tests can see the real terminal, and spawning `clear` there would
        // clear the user's console during `cargo test`.
        // 没有捕获守卫时命令仍然绝不执行：单元测试能看到真实终端，在测试
        // 期间 spawn `clear` 会清掉用户控制台。
        clear_screen();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn clear_command_status_runs_the_platform_command() {
        let status =
            clear_command_status(std::process::Stdio::piped(), std::process::Stdio::piped())
                .expect("run cls");
        assert!(status.success());
    }

    #[test]
    fn sync_global_config_reports_write_failure() {
        let root = crate::test_support::temp_dir("sync-fail");
        // A regular file at the `.ManualAid` path makes the config write fail.
        std::fs::write(root.join(".ManualAid"), "occupied").unwrap();
        let messages = sync_global_config(&root, &Config::default());
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("cannot create config directory"));
    }

    #[test]
    fn format_round_summary_keeps_slice_whitespace() {
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let summary = format_round_summary(&[ToolResult::success(
            "read",
            "    indented  \n  second\n",
            true,
        )]);
        assert!(summary.contains("      indented  "));
        assert!(summary.contains("    second"));
    }

    #[test]
    fn format_config_issue_warns_about_dangerous_allow_command() {
        i18n::set_locale("en");
        let issue = ConfigIssue {
            kind: ConfigIssueKind::DangerousAllowCommand,
            key: "permissions.allow_commands".into(),
            value: "rm *".into(),
            available_values: Vec::new(),
            path: std::path::PathBuf::from("project.toml"),
        };
        let message = format_config_issue(&issue);
        assert!(message.contains("rm *"));
        assert!(message.contains("project.toml"));
        assert!(message.contains("ignored"));
    }

    #[test]
    fn format_config_issue_reports_invalid_value() {
        i18n::set_locale("en");
        let issue = ConfigIssue {
            kind: ConfigIssueKind::InvalidValue,
            key: "lang".into(),
            value: "fr".into(),
            available_values: vec!["en".into(), "zh-CN".into()],
            path: std::path::PathBuf::from("global.toml"),
        };
        let message = format_config_issue(&issue);
        assert!(message.contains("lang"));
        assert!(message.contains("fr"));
        assert!(message.contains("en, zh-CN"));
    }
}
