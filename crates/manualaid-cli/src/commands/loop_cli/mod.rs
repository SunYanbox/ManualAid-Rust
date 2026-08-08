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
use manualaid_core::skill::reload_skills_with_home;
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
    apply_cli_lang, apply_format_mode, clear_screen, read_line, sync_global_config, t_fmt,
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
    loop_main_at(&current_dir, &home, lang).await
}

/// The interactive loop body against explicit project and home directories
/// so tests can run it on a temporary workspace.
/// 面向显式项目目录与主目录的交互式 loop 主体，测试可在临时工作区运行。
async fn loop_main_at(current_dir: &Path, home: &Path, lang: Option<String>) -> Result<(), String> {
    let (mut config, issues) = manualaid_ws::config::load(current_dir, home)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    // Snapshot for the startup hint: the CLI language override below must
    // not count as a config-file change.
    // 快照用于启动提示：下方的 CLI 语言覆盖不应被算作配置文件更改。
    let loaded_config = config.clone();
    apply_cli_lang(lang, &mut config);
    i18n::set_locale(&config.lang);

    // Print validation warnings for invalid config values
    // 打印配置验证警告
    for issue in &issues {
        println!(
            "{}",
            t_fmt(
                "cli.warning.invalid_config_value",
                &[
                    ("key", &issue.key),
                    ("value", &issue.value),
                    ("available", &issue.available_values.join(", ")),
                    ("path", &issue.path.display().to_string()),
                ]
            )
        );
    }

    reload_skills_with_home(current_dir, home).map_err(|e| e.to_string())?;

    let registry = FormatRegistry::new();
    apply_format_mode(&registry, &config)?;

    let auditor = Auditor::new(current_dir.to_path_buf())
        .with_allowed_commands(config.allow_commands.clone());
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
    // config, and tell the user when a config file changed any `[global]`
    // default. The write failure never aborts the loop.
    // 把生效的字符限额写入项目配置，使其始终可见可改；配置文件改动过
    // 任一 `[global]` 默认值时，在启动时逐条告知用户。写入失败不会中止
    // loop。
    for message in sync_global_config(current_dir, &loaded_config) {
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
            handle_inline_command(&mut config, &registry, current_dir, &mut session, trimmed);
            continue;
        }
        match trimmed {
            "1" => copy_system_prompt(&config, current_dir, &registry),
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
            "5" => config_menu(&mut config, &registry, current_dir, &mut options),
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
    use super::utils::push_test_input;
    use super::*;

    use indexmap::IndexMap;
    use manualaid_core::audit::{AuditDecision, AuditQueueItem};
    use manualaid_core::tools::ToolResult;
    use manualaid_ws::config::Config;
    use serde_json::Value;

    #[test]
    fn sync_global_config_writes_default_without_hints() {
        i18n::set_locale("en");
        let root = std::env::temp_dir().join(format!("manualaid-cli-sync-{}", std::process::id()));
        let messages = sync_global_config(&root, &Config::default());
        assert!(messages.is_empty());
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("max_result_chars = 50000"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_global_config_hints_each_changed_global_value() {
        i18n::set_locale("en");
        let root =
            std::env::temp_dir().join(format!("manualaid-cli-sync-{}-v", std::process::id()));
        let config = Config {
            lang: "zh-CN".to_string(),
            tool_call_format: "xml".to_string(),
            max_result_chars: 123_456,
            ..Config::default()
        };
        let messages = sync_global_config(&root, &config);
        assert_eq!(messages.len(), 3);
        let path = root
            .join(".ManualAid")
            .join("config.toml")
            .display()
            .to_string();
        // Key and value strings are identical in both locales, so these
        // assertions survive the process-wide locale races with other tests.
        // 键名与值在两种语言下完全相同，断言不受其他测试切换 locale 的影响。
        assert!(messages[0].contains(&path));
        assert!(messages[0].contains("lang = \"zh-CN\""));
        assert!(messages[1].contains("tool_call_format = \"xml\""));
        assert!(messages[2].contains("max_result_chars = 123456"));
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("max_result_chars = 123456"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn sync_global_config_hints_only_changed_values() {
        i18n::set_locale("en");
        let root =
            std::env::temp_dir().join(format!("manualaid-cli-sync-{}-p", std::process::id()));
        let config = Config {
            lang: "zh-CN".to_string(),
            ..Config::default()
        };
        let messages = sync_global_config(&root, &config);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("lang = \"zh-CN\""));
        assert!(!messages[0].contains("tool_call_format"));
        assert!(!messages[0].contains("max_result_chars"));
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
    fn format_round_summary_shows_approval_reasons() {
        i18n::set_locale("en");
        let mut result = ToolResult::success("shell", "done", true);
        result.audit_decisions.push((
            "command".to_string(),
            AuditDecision::NeedsApproval("needs review".into()),
        ));
        let summary = format_round_summary(&[result]);
        assert!(summary.contains("command (needs review)"));
    }

    #[test]
    fn apply_format_mode_rejects_unknown_label() {
        let registry = FormatRegistry::new();
        let config = Config {
            tool_call_format: "bogus".to_string(),
            ..Config::default()
        };
        assert!(apply_format_mode(&registry, &config).is_err());
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

    #[tokio::test]
    async fn loop_main_at_drives_menu_flow_with_scripted_input() {
        let _lang_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skill_lock = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let current_dir = crate::test_support::temp_dir("loop-main");
        let home = crate::test_support::temp_dir("loop-main-home");
        // A valid non-default lang triggers the sync hint; an invalid format
        // label produces a validation warning at startup.
        // 合法但非默认的 lang 触发同步提示；非法的 format 标签在启动时产生
        // 验证警告。
        std::fs::create_dir_all(current_dir.join(".ManualAid")).unwrap();
        std::fs::write(
            current_dir.join(".ManualAid").join("config.toml"),
            "[global]\nlang = \"zh-CN\"\ntool_call_format = \"bogus\"\n",
        )
        .unwrap();
        let file = current_dir.join("target.txt");
        std::fs::write(&file, "hello").unwrap();
        // 4: copy with no rounds (no clipboard access), 5: config menu
        // (9 toggles clear_screen, 0 exits), 3: typed round, 6: summary,
        // x: invalid option; the queue then runs dry, ending the loop on
        // stdin EOF.
        // 4：无轮次复制（不触碰剪贴板），5：配置菜单（9 切换清屏，0 返回），
        // 3：手动输入一轮，6：摘要，x：非法选项；随后队列耗尽，以 stdin EOF
        // 结束循环。
        push_test_input(&[
            "/tools",
            "/format 2",
            "4",
            "5",
            "9",
            "0",
            "3",
            format!("<read><file_path>{}</file_path></read>", file.display()).as_str(),
            "/end",
            "n",
            "6",
            "x",
        ]);
        loop_main_at(&current_dir, &home, None).await.unwrap();
        // The /format inline command persists its change; the pre-written
        // lang stays untouched.
        let content =
            std::fs::read_to_string(current_dir.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("lang = \"zh-CN\""));
        assert!(content.contains("tool_call_format = \"xml\""));
        manualaid_core::skill::reset_skills();
    }

    #[test]
    fn run_loop_works_against_explicit_and_real_home() {
        let _cwd_lock = crate::test_support::CWD_LOCK.lock().unwrap();
        let _lang_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skill_lock = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let original = std::env::current_dir().unwrap();
        let cwd = crate::test_support::temp_dir("run-loop-cwd");
        std::env::set_current_dir(&cwd).unwrap();
        let home = crate::test_support::temp_dir("run-loop-home");
        push_test_input(&["0"]);
        run_loop(Some(&home), None).unwrap();
        // The home_dir() fallback reads the real user home, so no temp home
        // is passed here; the loop only reads it.
        // home_dir() 回退读取真实用户主目录，此处不传临时主目录；loop 只读它。
        push_test_input(&["0"]);
        run_loop(None, None).unwrap();
        std::env::set_current_dir(&original).unwrap();
        assert!(cwd.join(".ManualAid").join("config.toml").is_file());
        manualaid_core::skill::reset_skills();
    }
}
