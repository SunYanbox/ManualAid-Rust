//! Small utilities for the loop: formatting, input and cycling helpers.
//! loop 的小工具函数：格式化、输入与循环切换辅助。

use std::path::Path;

use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::tools::ToolResult;
use manualaid_ws::config::Config;

/// Translate `key` and replace `%{name}` placeholders.
/// 翻译 `key` 并替换 `%{name}` 占位符。
pub(super) fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
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
        return TEST_INPUT.with(|input| input.borrow_mut().pop_front());
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
        std::cell::RefCell::new(std::collections::VecDeque::new());
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

/// Clear the screen via the platform command; failures are ignored.
/// 通过平台命令清屏；失败时静默忽略。
pub(super) fn clear_screen() {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "cls"])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("clear")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }
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
    [
        "cli.loop.menu_title",
        "cli.loop.menu_generate",
        "cli.loop.menu_paste",
        "cli.loop.menu_input",
        "cli.loop.menu_copy",
        "cli.loop.menu_config",
        "cli.loop.menu_summary",
        "cli.loop.menu_exit",
    ]
    .iter()
    .map(|key| i18n::t_str(key))
    .collect::<Vec<_>>()
    .join("\n")
        + "\n"
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
            i18n::t_str("cli.message.success")
        } else {
            i18n::t_str("cli.message.failure")
        };
        out.push_str(&format!(
            "[{}] {state}\n{}\n",
            result.tool_name,
            result.output.trim()
        ));
        for (param, decision) in &result.audit_decisions {
            if let Some(reason) = decision.reason() {
                out.push_str(&format!(
                    "  {}: {param} ({reason})\n",
                    i18n::t_str("cli.message.approval_needed")
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_line_consumes_scripted_input_then_eof() {
        push_test_input(&["first", "second"]);
        assert_eq!(read_line().as_deref(), Some("first"));
        assert_eq!(read_line().as_deref(), Some("second"));
        assert_eq!(read_line(), None);
    }

    #[test]
    fn clear_screen_runs_platform_command() {
        clear_screen();
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
}
