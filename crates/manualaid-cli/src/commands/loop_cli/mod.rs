//! The interactive Agent Copy-Paste Loop: generate a workspace system
//! prompt, parse and execute tool-call text pasted from (or typed into)
//! the console with per-item audit approvals, and copy the results back to
//! the clipboard for an external LLM chat.
//! 交互式 Agent Copy-Paste Loop：生成工作区系统提示词，解析并执行从
//! 剪贴板粘贴或手动输入的工具调用文本（带逐项审计批准），并把结果复制
//! 回剪贴板供外部 LLM 聊天使用。

use std::path::Path;
use std::sync::Arc;

use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::reload_skills_with_home;
use manualaid_core::user_dir::home_dir;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

mod approval;
mod bang;
mod command;
mod complete;
mod config;
mod context;
mod diff;
mod handlers;
mod inline;
mod menu;
mod path_action;
mod preview;
mod skill_action;
pub(crate) mod utils;

pub use approval::execute_round_with_approval;
pub use config::{render_config_menu, render_tool_menu};
pub use preview::approval_preview;
pub use utils::{
    cycle_format, cycle_lang, format_round_detail, format_round_header, format_round_summary,
    parse_round_index, push_test_input, render_menu,
};
// Handlers are re-exported for the integration tests in
// tests/commands/handlers.rs; the no-provider wrappers stay private.
// handlers 重导出供 tests/commands/handlers.rs 集成测试使用；
// 不带 provider 的薄包装保持私有。
pub use handlers::{
    ask_copy, copy_compressed_session_prompt_with_provider,
    copy_context_with_context_files_with_provider, copy_context_with_provider,
    copy_enabled_tools_with_provider, copy_intent_rule_with_provider,
    copy_line_ending_rule_with_provider, copy_plan_mode_rule_with_provider, copy_round_result,
    copy_round_result_with_provider, copy_switch_mode_rule_with_provider,
    copy_system_prompt_with_context_files_with_provider, copy_system_prompt_with_provider,
    copy_task_planning_rule_with_provider, copy_tool_format_with_provider, input_and_submit,
    paste_and_submit_with_provider, print_session_summary, show_tool_history, submit_text,
    submit_text_with_provider, truncate_preview_lines,
};

use command::{CommandOutcome, run_command};
use complete::candidates::{Candidate, filter as filter_candidates};
use complete::editor::read_line_with_completion;
use complete::paths::PathCandidates;
use complete::state::{CompletionState, InputHistory, Trigger};
use inline::handle_inline_command;
use menu::{MenuAction, build_main_menu};
use utils::{clear_screen, mode_hint, sync_global_config};
// Re-exported for the sibling `copy` subcommand, which reuses the same
// initialization steps as the interactive loop.
// 为同级的 `copy` 子命令重新导出，使其复用交互式 loop 的初始化步骤。
pub(crate) use utils::{apply_cli_lang, apply_format_mode, format_config_issue, t_fmt};

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
    /// Whether Edit/Write inside the workspace auto-approve.
    /// 工作区内的 Edit/Write 是否自动放行。
    pub mode: SessionMode,
}

impl Default for LoopOptions {
    fn default() -> Self {
        Self {
            auto_copy: true,
            clear_screen: false,
            mode: SessionMode::Manual,
        }
    }
}

/// Run the interactive loop with a new Tokio runtime; the caller remains
/// synchronous.
/// 用新建的 Tokio runtime 运行交互式 loop；调用方保持同步。
pub fn run_loop(
    home: Option<&Path>,
    lang: Option<String>,
    mode: Option<SessionMode>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build runtime: {e}"))?;
    runtime.block_on(loop_main(home, lang, mode))
}

/// The interactive loop body (async because tool execution is async).
/// 交互式 loop 主体（异步，因为工具执行是异步的）。
async fn loop_main(
    home: Option<&Path>,
    lang: Option<String>,
    mode: Option<SessionMode>,
) -> Result<(), String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))?;
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir().map_err(|e| e.to_string())?,
    };
    loop_main_at(&current_dir, &home, lang, mode).await
}

/// The interactive loop body against explicit project and home directories
/// so tests can run it on a temporary workspace.
/// 面向显式项目目录与主目录的交互式 loop 主体，测试可在临时工作区运行。
async fn loop_main_at(
    current_dir: &Path,
    home: &Path,
    lang: Option<String>,
    mode: Option<SessionMode>,
) -> Result<(), String> {
    // Title the window first so parallel sessions on different projects are
    // distinguishable before any output scrolls past.
    // 先设置窗口标题，让不同项目的并行会话在任何输出滚过之前就可区分。
    crate::terminal_title::set_project_title(current_dir);
    // Create the standard project `.ManualAid` files (config + gitignore)
    // at startup so a fresh workspace never lacks its ignore rules;
    // existing files are never overwritten.
    // 启动时创建标准的项目 `.ManualAid` 文件（配置与 .gitignore），
    // 避免新工作区缺少忽略规则；已有文件不会被覆盖。
    manualaid_core::manualaid_dir::ensure_project_manualaid_dir(current_dir)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    let (mut config, issues) = manualaid_ws::config::load(current_dir, home)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    // Snapshot for the startup hint: the CLI language override below must
    // not count as a config-file change.
    // 快照用于启动提示：下方的 CLI 语言覆盖不应被算作配置文件更改。
    let loaded_config = config.clone();
    apply_cli_lang(lang, &mut config);
    i18n::set_locale(&config.lang);

    // Print config validation warnings (invalid values, dangerous
    // whitelist entries that were ignored).
    // 打印配置验证警告（无效值、被忽略的危险白名单条目）。
    for issue in &issues {
        crate::console::out_println!("{}", format_config_issue(issue));
    }

    reload_skills_with_home(current_dir, home).map_err(|e| e.to_string())?;

    let registry = FormatRegistry::new();
    apply_format_mode(&registry, &config)?;
    // 解析器只识别会话中启用的工具，集合不变时复用注册表缓存。
    registry
        .set_enabled_tools(&config.enabled_tool_names())
        .map_err(|e| e.to_string())?;

    let mode = mode.unwrap_or_default();
    let mut executor = build_executor(current_dir, &config, mode);
    let mut session = SessionLog::new();
    let mut options = LoopOptions {
        mode,
        ..LoopOptions::default()
    };

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let shell = manualaid_core::shell::detected_shell();
    crate::console::out_println!(
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
        crate::console::out_println!("{message}");
    }

    let main_menu = build_main_menu();
    let path_cache = Arc::new(std::sync::Mutex::new(PathCandidates::default()));
    path_cache.lock().unwrap().scan(current_dir);
    let input_history = Arc::new(InputHistory::new());
    let mut should_exit = false;
    let mut show_help_hint = true;
    while !should_exit {
        if options.clear_screen {
            clear_screen();
        }
        let _ = crate::pager::print_paged(&render_menu());
        // Remind once, after the first menu render, that inline commands
        // are available behind `/help`.
        // 首次渲染菜单后提示一次：内联命令可通过 `/help` 查看。
        if show_help_hint {
            crate::console::out_println!(
                "{}",
                crate::style::muted(&i18n::t_str("cli.loop.help_hint"))
            );
            show_help_hint = false;
        }
        let prompt = format!(
            "{} {}",
            mode_hint(options.mode),
            i18n::t_str("cli.loop.menu_prompt")
        );
        let path_cache = path_cache.clone();
        #[cfg(test)]
        let line = {
            let root = current_dir;
            match read_line_with_completion(&prompt, Some(input_history.clone()), |state| {
                completion_candidates(state, root, &path_cache)
            }) {
                Some(line) => line,
                None => break,
            }
        };
        #[cfg(not(test))]
        let line = {
            let root = current_dir.to_path_buf();
            let input_history = input_history.clone();
            match tokio::task::spawn_blocking(move || {
                read_line_with_completion(&prompt, Some(input_history), |state| {
                    completion_candidates(state, &root, &path_cache)
                })
            })
            .await
            {
                Ok(Some(line)) => line,
                _ => break,
            }
        };
        let trimmed = line.trim();

        if trimmed.starts_with('/') {
            let mode_before = options.mode;
            let skill_handled = skill_action::run_skill_action(
                &manualaid_core::clipboard::RealClipboard,
                &executor,
                current_dir,
                &mut config,
                &mut session,
                &mut options,
                trimmed,
            )
            .await;
            if options.mode != mode_before {
                executor = build_executor(current_dir, &config, options.mode);
            }
            if skill_handled {
                continue;
            }
            handle_inline_command(
                &mut config,
                &registry,
                current_dir,
                &mut session,
                &mut options,
                trimmed,
            );
            if options.mode != mode_before {
                executor = build_executor(current_dir, &config, options.mode);
            }
            continue;
        }

        if trimmed.contains('@') {
            let mode_before = options.mode;
            let path_handled = path_action::run_path_actions(
                &manualaid_core::clipboard::RealClipboard,
                &executor,
                current_dir,
                &mut config,
                &mut session,
                &mut options,
                trimmed,
            )
            .await;
            if options.mode != mode_before {
                executor = build_executor(current_dir, &config, options.mode);
            }
            if path_handled {
                continue;
            }
        }

        if trimmed.starts_with('!') {
            let mode_before = options.mode;
            bang::run_bang_command(
                &manualaid_core::clipboard::RealClipboard,
                &executor,
                current_dir,
                &mut config,
                &mut session,
                &mut options,
                trimmed,
            )
            .await;
            if options.mode != mode_before {
                executor = build_executor(current_dir, &config, options.mode);
            }
            continue;
        }

        let Some(action) = main_menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            if !should_exit && options.clear_screen {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            continue;
        };
        let command = match action {
            MenuAction::Command(command) => command,
            MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        if matches!(command, command::LoopCommand::ConfigMenu) {
            let mode_before = options.mode;
            config::config_menu(
                &manualaid_core::clipboard::RealClipboard,
                &mut config,
                &registry,
                current_dir,
                &mut options,
                &mut session,
            )
            .await;
            if options.mode != mode_before {
                executor = build_executor(current_dir, &config, options.mode);
            }
            continue;
        }
        if matches!(command, command::LoopCommand::CopyPromptMenu) {
            config::copy_prompt_menu(
                &manualaid_core::clipboard::RealClipboard,
                &config,
                &registry,
                current_dir,
            )
            .await;
            continue;
        }
        let mode_before = options.mode;
        let provider = manualaid_core::clipboard::RealClipboard;
        let mut ctx = command::CommandContext {
            provider: &provider,
            executor: &executor,
            registry: &registry,
            config: &mut config,
            options: &mut options,
            root: current_dir,
            session: &mut session,
        };
        match run_command(command, &mut ctx).await {
            CommandOutcome::Continue => {}
            CommandOutcome::Exit => {}
            CommandOutcome::ExitLoop => should_exit = true,
        }
        if !should_exit && options.mode != mode_before {
            executor = build_executor(current_dir, &config, options.mode);
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

/// Build an executor with the given session approval mode.
/// 按给定的会话审批模式构建执行器。
fn build_executor(root: &Path, config: &Config, mode: SessionMode) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf())
            .with_allowed_commands(config.allow_commands.clone())
            .with_mode(mode),
        Arc::new(None),
    )
}

/// Build the completion candidates for the current active token. Command
/// tokens merge built-in commands with enabled skills; path tokens read from
/// the session-cached path scanner.
/// 为当前活动 token 构建补全候选。命令 token 合并内置命令与已启用技能；
/// 路径 token 从会话缓存的路径扫描器读取。
fn completion_candidates(
    state: &CompletionState,
    _root: &Path,
    path_cache: &Arc<std::sync::Mutex<PathCandidates>>,
) -> Vec<Candidate> {
    let Some(query) = state.active_query() else {
        return Vec::new();
    };
    match state.active_trigger() {
        Some(Trigger::Command) => {
            // Command labels already carry the leading `/`; skill labels are
            // stable unique names such as `project-.agents-demo`, so the
            // skill query must drop the `/` typed at the prompt. Matching the
            // two groups separately keeps skills visible after the command
            // candidates when the user scrolls.
            // 命令候选自带起始 `/`；技能候选是 `project-.agents-demo` 这类
            // 稳定唯一名，所以技能查询需去掉提示符处输入的 `/`。分组匹配
            // 可让技能候选在用户滚动时出现在命令候选之后。
            let skill_query = query.strip_prefix('/').unwrap_or(query).to_lowercase();
            let mut matches = filter_candidates(complete::candidates::builtin_commands(), query);
            // Skill labels are stable unique names such as
            // `project-.agents-commit-objectively`; search them by substring so
            // typing `commit` matches even though the label does not start with
            // that word.
            // 技能候选是 `project-.agents-commit-objectively` 这类稳定唯一名；
            // 按子串匹配，输入 `commit` 时虽不以该词开头也应命中。
            matches.extend(
                complete::candidates::skills()
                    .into_iter()
                    .filter(|candidate| candidate.label.to_lowercase().contains(&skill_query)),
            );
            matches
        }
        Some(Trigger::Path) => {
            let mut cache = path_cache.lock().unwrap();
            cache
                .filter(query)
                .into_iter()
                .map(|entry| Candidate {
                    label: entry.label,
                    description: String::new(),
                    is_dir: entry.is_dir,
                    dimmed: entry.dimmed,
                })
                .collect()
        }
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use complete::state::Key;

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
        assert_eq!(cycle_format("auto"), "json-codeblock");
        assert_eq!(cycle_format("json-codeblock"), "invoke");
        assert_eq!(cycle_format("invoke"), "xml");
        assert_eq!(cycle_format("xml"), "auto");
        assert_eq!(cycle_format("bogus"), "json-codeblock");
    }

    #[test]
    fn format_round_summary_shows_state_and_output() {
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let results = vec![
            ToolResult::success("read", "hello", true),
            ToolResult::failure("edit", "boom"),
        ];
        let summary = format_round_summary(&results);
        assert!(summary.contains("[read]"));
        assert!(summary.contains("success"));
        assert!(summary.contains("hello"));
        assert!(summary.contains("[edit]"));
        assert!(summary.contains("failure"));
        assert!(summary.contains("boom"));
    }

    #[test]
    fn format_round_summary_shows_approval_reasons() {
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
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
    fn format_round_summary_styles_blocks_when_enabled() {
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        crate::style::set_enabled(true);
        let summary = format_round_summary(&[
            ToolResult::success("read", "hello", true),
            ToolResult::failure("edit", "boom"),
        ]);
        assert!(summary.contains("\x1b["));
        let plain = crate::style::strip_ansi(&summary);
        assert!(plain.contains("[read] success"));
        assert!(plain.contains("  hello"));
        assert!(plain.contains("[edit] failure"));
        assert!(plain.contains("  boom"));
        crate::style::set_enabled(false);
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
    fn completion_candidates_command_merges_builtin_commands_and_skills() {
        let _locale = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("completion-candidates-command");
        let home = crate::test_support::temp_dir("completion-candidates-command-home");
        let skill_dir = root.join(".claude").join("skills").join("demo");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\nbody\n",
        )
        .unwrap();
        manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();

        let mut state = CompletionState::new();
        for ch in "/demo".chars() {
            state.handle_key(Key::Char(ch));
        }
        let cache = Arc::new(std::sync::Mutex::new(PathCandidates::default()));
        let candidates = completion_candidates(&state, &root, &cache);

        // The typed `/demo` matches no built-in command prefix, so the only
        // hit is the enabled skill matched by substring on its unique name.
        // 键入的 `/demo` 不匹配任何内置命令前缀，唯一命中是按唯一名子串
        // 匹配到的已启用技能。
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "project-.claude-demo");

        // Leave the shared skill store empty for the other tests.
        // 为其他测试把共享技能存储清空。
        let empty = crate::test_support::temp_dir("completion-candidates-empty");
        manualaid_core::skill::reload_skills_with_home(&empty, &empty).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn completion_candidates_path_reads_the_session_cache() {
        let root = crate::test_support::temp_dir("completion-candidates-path");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("main.rs"), "x").unwrap();
        let mut cache = PathCandidates::default();
        cache.scan(&root);
        let cache = Arc::new(std::sync::Mutex::new(cache));

        let mut state = CompletionState::new();
        for ch in "@src".chars() {
            state.handle_key(Key::Char(ch));
        }
        let candidates = completion_candidates(&state, &root, &cache);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "src");
        assert!(candidates[0].is_dir);
        assert!(!candidates[0].dimmed);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn completion_candidates_without_an_active_query_is_empty() {
        let cache = Arc::new(std::sync::Mutex::new(PathCandidates::default()));
        let candidates = completion_candidates(&CompletionState::new(), Path::new("."), &cache);
        assert!(candidates.is_empty());
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

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn loop_warns_when_allow_commands_match_the_blacklist() {
        let _capture = crate::console::capture();
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("loop-dangerous-ws");
        let home = crate::test_support::temp_dir("loop-dangerous-home");
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        std::fs::write(
            root.join(".ManualAid").join("config.toml"),
            "[permissions]\nallow_commands = [\"git log *\", \"rm *\"]\n",
        )
        .unwrap();
        super::utils::push_test_input(&["0"]);
        loop_main_at(&root, &home, None, None).await.unwrap();
        let output = _capture.text();
        assert!(output.contains("rm *"));
        assert!(output.contains("ignored"));
        assert!(!output.contains("git log *"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn loop_path_token_branch_continues_the_loop() {
        // A line containing `@` is routed through the path-action branch;
        // a missing target prints the localized hint and the loop resumes.
        // 含 `@` 的行会进入路径操作分支；目标不存在时打印本地化提示，
        // loop 继续运行。
        let _capture = crate::console::capture();
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("loop-path-token-ws");
        let home = crate::test_support::temp_dir("loop-path-token-home");
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        super::utils::push_test_input(&["@nonexistent_zzz", "0"]);
        loop_main_at(&root, &home, None, None).await.unwrap();
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn loop_mode_toggle_switches_session_mode() {
        let _capture = crate::console::capture();
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("loop-mode-ws");
        let home = crate::test_support::temp_dir("loop-mode-home");
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        super::utils::push_test_input(&["/mode", "0"]);
        loop_main_at(&root, &home, None, None).await.unwrap();
        let output = _capture.text();
        // Single toggle covers both branches: Manual -> AcceptEdit
        assert!(output.contains("accept edit"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn loop_titles_the_window_with_the_project_folder() {
        let capture = crate::console::capture();
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("loop-title-ws");
        let home = crate::test_support::temp_dir("loop-title-home");
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        super::utils::push_test_input(&["0"]);
        loop_main_at(&root, &home, None, None).await.unwrap();
        // The title must be the very first thing the loop writes, so it lands
        // before any output scrolls past.
        // 标题必须是 loop 写出的第一样东西，才能在任何输出滚过之前生效。
        let folder = root.file_name().unwrap().to_string_lossy();
        assert!(
            capture
                .text()
                .starts_with(&format!("\x1B]0;[ManualAid] {folder}\x07"))
        );
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn loop_exit_command_handling() {
        // 覆盖 mod.rs:265-266 - 主循环中 exit 命令路径
        let _capture = crate::console::capture();
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("loop-exit-ws");
        let home = crate::test_support::temp_dir("loop-exit-home");
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        // 输入 "0" 对应退出命令（主菜单的 0 是 exit）
        super::utils::push_test_input(&["0"]);
        let result = loop_main_at(&root, &home, None, None).await;
        assert!(result.is_ok());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn loop_config_load_error_handling() {
        // 覆盖 mod.rs:298 - 配置加载错误处理
        // 通过尝试在无效目录加载配置来触发错误
        let root = crate::test_support::temp_dir("config-load-error");
        let home = crate::test_support::temp_dir("config-load-error-home");
        // 创建 config.toml 但内容无效
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        std::fs::write(
            root.join(".ManualAid").join("config.toml"),
            "invalid = toml [ content",
        )
        .unwrap();
        let result = manualaid_ws::config::load(&root, &home);
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn config_validation_warning_handling() {
        // 覆盖 mod.rs:302 - 配置验证警告打印
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let path = std::path::PathBuf::from("/fake/path/config.toml");
        let issues = vec![
            manualaid_ws::config::ConfigIssue {
                kind: manualaid_ws::config::ConfigIssueKind::InvalidValue,
                key: "tool_call_format".to_string(),
                value: "bogus".to_string(),
                available_values: vec!["auto".to_string(), "xml".to_string()],
                path: path.clone(),
            },
            manualaid_ws::config::ConfigIssue {
                kind: manualaid_ws::config::ConfigIssueKind::DangerousAllowCommand,
                key: "allow_commands".to_string(),
                value: "rm -rf /".to_string(),
                available_values: vec![],
                path,
            },
        ];
        for issue in &issues {
            crate::console::out_println!("{}", format_config_issue(issue));
        }
        let output = _capture.text();
        assert!(output.contains("bogus") || output.contains("rm"));
    }
}
