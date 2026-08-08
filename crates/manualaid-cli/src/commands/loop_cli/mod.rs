//! The interactive Agent Copy-Paste Loop: generate a workspace system
//! prompt, parse and execute tool-call text pasted from (or typed into)
//! the console with per-item audit approvals, and copy the results back to
//! the clipboard for an external LLM chat.
//! 交互式 Agent Copy-Paste Loop：生成工作区系统提示词，解析并执行从
//! 剪贴板粘贴或手动输入的工具调用文本（带逐项审计批准），并把结果复制
//! 回剪贴板供外部 LLM 聊天使用。

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use manualaid_core::audit::Auditor;
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::reload_skills;
use manualaid_core::user_dir::home_dir;
use manualaid_ws::session::SessionLog;

mod approval;
mod config;
mod handlers;
mod inline;
mod preview;
mod utils;

pub use approval::execute_round_with_approval;
pub use config::render_config_menu;
pub use preview::approval_preview;
pub use utils::{cycle_format, cycle_lang, format_round_summary, parse_round_index, render_menu};

use config::config_menu;
use handlers::{
    copy_round_result, copy_system_prompt, input_and_submit, paste_and_submit,
    print_session_summary,
};
use inline::handle_inline_command;
use utils::{
    apply_cli_lang, apply_format_mode, clear_screen, read_line, sync_max_result_chars, t_fmt,
};

/// How the user answered one approval-queue item.
/// 用户对单个审批队列项的答复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approval {
    /// Approve the operation.
    /// 同意该操作。
    Approve,
    /// Deny the operation.
    /// 拒绝该操作。
    Deny,
    /// Deny the operation and return the typed text as the tool result.
    /// 拒绝该操作，并把键入的文本作为工具调用结果返回。
    DenyWithText(String),
}

/// Session-level loop switches (not persisted).
/// 会话级 loop 开关（不持久化）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopOptions {
    /// Whether results are copied to the clipboard automatically after an
    /// executed round.
    /// 每轮执行后是否自动把结果复制到剪贴板。
    pub auto_copy: bool,
    /// Whether the screen is cleared before each menu render.
    /// 每次渲染菜单前是否清屏。
    pub clear_screen: bool,
}

impl Default for LoopOptions {
    fn default() -> Self {
        Self {
            auto_copy: true,
            clear_screen: false,
        }
    }
}

/// Run the interactive loop with a new Tokio runtime; the caller remains
/// synchronous.
/// 用新建的 Tokio runtime 运行交互式 loop；调用方保持同步。
pub fn run_loop(home: Option<&Path>, lang: Option<String>) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build runtime: {e}"))?;
    runtime.block_on(loop_main(home, lang))
}

/// The interactive loop body (async because tool execution is async).
/// 交互式 loop 主体（异步，因为工具执行是异步的）。
async fn loop_main(home: Option<&Path>, lang: Option<String>) -> Result<(), String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))?;
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir().map_err(|e| e.to_string())?,
    };

    let mut config = manualaid_ws::config::load(&current_dir, &home)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    apply_cli_lang(lang, &mut config);
    i18n::set_locale(&config.lang);

    reload_skills(&current_dir).map_err(|e| e.to_string())?;

    let registry = FormatRegistry::new();
    apply_format_mode(&registry, &config)?;

    let auditor =
        Auditor::new(current_dir.clone()).with_allowed_commands(config.allow_commands.clone());
    let executor = Executor::new(auditor, Arc::new(None));
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let shell = manualaid_core::shell::detected_shell();
    println!(
        "{}",
        t_fmt(
            "cli.loop.header",
            &[
                ("path", &current_dir.display().to_string()),
                ("time", &now),
                ("shell", &shell),
            ],
        )
    );

    // Keep the effective character limit visible and editable in the project
    // config, and tell the user when a config file changed the default.
    // 把生效的字符限额写入项目配置，使其始终可见可改；配置文件中改过
    // 默认值时，在启动时告知用户。
    if let Some(message) = sync_max_result_chars(&current_dir, config.max_result_chars) {
        println!("{message}");
    }

    let mut should_exit = false;
    while !should_exit {
        if options.clear_screen {
            clear_screen();
        }
        let _ = crate::pager::print_paged(&render_menu());
        print!("{}", i18n::t_str("cli.loop.menu_prompt"));
        let _ = std::io::stdout().flush();

        let line = match read_line() {
            Some(line) => line,
            None => break,
        };
        let trimmed = line.trim();
        if trimmed.starts_with('/') {
            handle_inline_command(&mut config, &registry, &current_dir, &mut session, trimmed);
            continue;
        }
        match trimmed {
            "1" => copy_system_prompt(&config, &current_dir, &registry),
            "2" => {
                paste_and_submit(
                    &executor,
                    &registry,
                    &mut session,
                    &mut options,
                    config.max_result_chars,
                )
                .await
            }
            "3" => {
                input_and_submit(
                    &executor,
                    &registry,
                    &mut session,
                    &mut options,
                    config.max_result_chars,
                )
                .await
            }
            "4" => copy_round_result(&session, config.max_result_chars),
            "5" => config_menu(&mut config, &registry, &current_dir, &mut options),
            "6" => print_session_summary(&config, &session),
            "0" => should_exit = true,
            _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
        }
        if !should_exit && options.clear_screen {
            // Keep the previous action's output readable before the next
            // screen clear.
            // 在下次清屏前保留上一步输出的可读时间。
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::preview::colorize_diff;
    use super::*;

    use indexmap::IndexMap;
    use manualaid_core::audit::{AuditDecision, AuditQueueItem};
    use manualaid_core::tools::ToolResult;
    use manualaid_ws::config::Config;
    use serde_json::Value;

    #[test]
    fn sync_max_result_chars_writes_default_without_hint() {
        i18n::set_locale("en");
        let root = std::env::temp_dir().join(format!("manualaid-cli-sync-{}", std::process::id()));
        let message = sync_max_result_chars(&root, Config::default().max_result_chars);
        assert!(message.is_none());
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("max_result_chars = 50000"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_max_result_chars_hints_when_changed_from_default() {
        i18n::set_locale("en");
        let root =
            std::env::temp_dir().join(format!("manualaid-cli-sync-{}-v", std::process::id()));
        let message = sync_max_result_chars(&root, 123_456).unwrap();
        assert!(message.contains("max_result_chars = 123456"));
        assert!(
            message.contains(
                &root
                    .join(".ManualAid")
                    .join("config.toml")
                    .display()
                    .to_string()
            )
        );
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("max_result_chars = 123456"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_round_index_defaults_to_latest() {
        assert_eq!(parse_round_index("", 3), Some(1));
        assert_eq!(parse_round_index("2", 3), Some(2));
        assert_eq!(parse_round_index("0", 3), None);
        assert_eq!(parse_round_index("4", 3), None);
        assert_eq!(parse_round_index("abc", 3), None);
    }

    #[test]
    fn cycle_lang_switches_between_two_locales() {
        assert_eq!(cycle_lang("en"), "zh-CN");
        assert_eq!(cycle_lang("zh-CN"), "en");
    }

    #[test]
    fn cycle_format_wraps_around() {
        assert_eq!(cycle_format("auto"), "xml");
        assert_eq!(cycle_format("xml"), "json-codeblock");
        assert_eq!(cycle_format("json-codeblock"), "auto");
        assert_eq!(cycle_format("bogus"), "xml");
    }

    #[test]
    fn format_round_summary_shows_state_and_output() {
        i18n::set_locale("en");
        let results = vec![
            ToolResult::success("read", "hello", true),
            ToolResult::failure("edit", "boom"),
        ];
        let summary = format_round_summary(&results);
        assert!(summary.contains("[read] success"));
        assert!(summary.contains("hello"));
        assert!(summary.contains("[edit] failure"));
        assert!(summary.contains("boom"));
    }

    #[test]
    fn approval_preview_shows_shell_command() {
        let item = AuditQueueItem {
            tool_name: "shell".into(),
            param_name: "command".into(),
            decision: AuditDecision::NeedsApproval("reason".into()),
        };
        let mut params = IndexMap::new();
        params.insert("command".to_string(), Value::String("git status".into()));
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("$ git status"));
        assert!(preview.contains("reason"));
    }

    #[test]
    fn approval_preview_includes_ai_description() {
        let item = AuditQueueItem {
            tool_name: "shell".into(),
            param_name: "command".into(),
            decision: AuditDecision::NeedsApproval("reason".into()),
        };
        let mut params = IndexMap::new();
        params.insert("command".to_string(), Value::String("git status".into()));
        params.insert(
            "description".to_string(),
            Value::String("check the repo state".into()),
        );
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("$ git status"));
        assert!(preview.contains("check the repo state"));
    }

    #[test]
    fn apply_cli_lang_overrides_config_and_ignores_invalid() {
        let mut config = Config::default();
        apply_cli_lang(Some("zh-CN".to_string()), &mut config);
        assert_eq!(config.lang, "zh-CN");
        apply_cli_lang(Some("fr".to_string()), &mut config);
        assert_eq!(config.lang, "zh-CN");
        apply_cli_lang(None, &mut config);
        assert_eq!(config.lang, "zh-CN");
    }

    #[test]
    fn approval_preview_raw_value_for_other_tools() {
        let item = AuditQueueItem {
            tool_name: "read".into(),
            param_name: "file_path".into(),
            decision: AuditDecision::NeedsApproval("outside".into()),
        };
        let mut params = IndexMap::new();
        params.insert("file_path".to_string(), Value::String("/etc/passwd".into()));
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("/etc/passwd"));
    }

    #[test]
    fn colorize_diff_keeps_plain_text_without_style() {
        crate::style::set_enabled(false);
        let colored = colorize_diff("@@ -1 +1 @@\n-a\n+b\n");
        assert!(!colored.contains("\x1b["));
        crate::style::set_enabled(true);
        let styled = colorize_diff("@@ -1 +1 @@\n-a\n+b\n");
        assert!(styled.contains("\x1b["));
        crate::style::set_enabled(false);
    }
}
