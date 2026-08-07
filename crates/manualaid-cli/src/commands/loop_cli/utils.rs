//! Small utilities for the loop: formatting, input and cycling helpers.
//! loop 的小工具函数：格式化、输入与循环切换辅助。

use std::io::BufRead;

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
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line),
        Err(_) => None,
    }
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
