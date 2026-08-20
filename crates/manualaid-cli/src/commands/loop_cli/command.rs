//! Unified loop commands shared by numeric menus and inline `/` commands.
//! 数字菜单与内置 `/` 命令共用的统一 loop 命令。

use std::path::{Path, PathBuf};

use manualaid_core::audit::SessionMode;
use manualaid_core::clipboard::ClipboardProvider;
use manualaid_core::executor::Executor;
use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::skill::{all_skills, set_enabled};
use manualaid_core::tools::ToolKind;
use manualaid_ws::config::{Config, save_project};
use manualaid_ws::session::SessionLog;

use super::LoopOptions;
use super::handlers::{
    copy_intent_rule_with_provider, copy_round_result_with_provider,
    copy_system_prompt_with_provider, input_and_submit, paste_and_submit_with_provider,
    print_session_summary, show_tool_history,
};
use super::utils::{
    apply_format_mode, cycle_format, cycle_lang, mode_label, print_muted_block, t_fmt,
};

/// The result of running a command.
/// 执行命令的结果。
pub(super) enum CommandOutcome {
    /// Keep the current menu loop running.
    /// 继续当前菜单循环。
    Continue,
    /// Exit the current menu loop.
    /// 退出当前菜单循环。
    Exit,
    /// Exit the main interactive loop.
    /// 退出主交互循环。
    ExitLoop,
}

/// Shared mutable and immutable state for executing one command.
/// 执行单个命令所需的共享可变与不可变状态。
pub(super) struct CommandContext<'a, P: ClipboardProvider> {
    pub provider: &'a P,
    pub executor: &'a Executor,
    pub registry: &'a FormatRegistry,
    pub config: &'a mut Config,
    pub options: &'a mut LoopOptions,
    pub root: &'a Path,
    pub session: &'a mut SessionLog,
}

/// Commands understood by both the numeric menus and inline commands.
/// 数字菜单与内置 inline 命令共同理解的命令。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoopCommand {
    GeneratePrompt,
    PasteAndSubmit,
    InputAndSubmit,
    CopyRoundResult,
    ConfigMenu,
    SessionSummary,
    ToolHistory,
    CopyIntentRule,
    Exit,
    ToggleMode,
    SwitchLang(Option<usize>),
    SwitchFormat(Option<usize>),
    ToggleShell,
    ToggleRead,
    ToggleWrite,
    ToggleEdit,
    ToggleSkill,
    ToggleAutoCopy,
    ToggleClearScreen,
    SkillMenu,
    ToggleContextAutoLoad,
    ShowMemoryUsage,
    EnableAllSkills,
    DisableAllSkills,
    ToggleSkillAt(PathBuf),
    Back,
}

/// Copy a tool call template to the clipboard.
/// 将工具调用模板复制到剪贴板。
pub(super) fn copy_tool_template<P: ClipboardProvider>(
    provider: &P,
    registry: &FormatRegistry,
    tool: &ToolKind,
) {
    match registry.render_tool_call_template(tool) {
        Ok(template) => match provider.write(&template) {
            Ok(()) => print_muted_block(&[i18n::t_str("cli.loop.copied")]),
            Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
        },
        Err(e) => eprintln!("{e}"),
    }
}

/// Run one loop command against the shared session state.
/// 在共享会话状态上执行一个 loop 命令。
pub(super) async fn run_command<P: ClipboardProvider>(
    cmd: &LoopCommand,
    ctx: &mut CommandContext<'_, P>,
) -> CommandOutcome {
    let provider = ctx.provider;
    let executor = ctx.executor;
    let registry = ctx.registry;
    let root = ctx.root;
    let config = &mut *ctx.config;
    let options = &mut *ctx.options;
    let session = &mut *ctx.session;
    match cmd {
        LoopCommand::GeneratePrompt => {
            copy_system_prompt_with_provider(provider, config, root, registry);
            CommandOutcome::Continue
        }
        LoopCommand::PasteAndSubmit => {
            paste_and_submit_with_provider(
                provider,
                executor,
                registry,
                session,
                options,
                config.max_result_chars,
            )
            .await;
            CommandOutcome::Continue
        }
        LoopCommand::InputAndSubmit => {
            input_and_submit(
                executor,
                registry,
                session,
                options,
                config.max_result_chars,
            )
            .await;
            CommandOutcome::Continue
        }
        LoopCommand::CopyRoundResult => {
            copy_round_result_with_provider(provider, session, config.max_result_chars);
            CommandOutcome::Continue
        }
        LoopCommand::ConfigMenu => {
            // Handled by the caller because entering this menu from inside
            // `run_command` would create an async recursion cycle.
            // 由调用方处理：若在这里进入菜单，会与菜单循环形成 async
            // 递归环。
            CommandOutcome::Continue
        }
        LoopCommand::SessionSummary => {
            print_session_summary(config, session);
            CommandOutcome::Continue
        }
        LoopCommand::ToolHistory => {
            show_tool_history(session);
            CommandOutcome::Continue
        }
        LoopCommand::CopyIntentRule => {
            copy_intent_rule_with_provider(provider);
            CommandOutcome::Continue
        }
        LoopCommand::Exit => CommandOutcome::ExitLoop,
        LoopCommand::ToggleMode => {
            toggle_mode(options);
            CommandOutcome::Continue
        }
        LoopCommand::SwitchLang(index) => {
            switch_lang(config, root, *index);
            CommandOutcome::Continue
        }
        LoopCommand::SwitchFormat(index) => {
            switch_format(registry, config, root, *index);
            CommandOutcome::Continue
        }
        LoopCommand::ToggleShell => {
            toggle_tool(config, root, "shell");
            CommandOutcome::Continue
        }
        LoopCommand::ToggleRead => {
            toggle_tool(config, root, "read");
            CommandOutcome::Continue
        }
        LoopCommand::ToggleWrite => {
            toggle_tool(config, root, "write");
            CommandOutcome::Continue
        }
        LoopCommand::ToggleEdit => {
            toggle_tool(config, root, "edit");
            CommandOutcome::Continue
        }
        LoopCommand::ToggleSkill => {
            toggle_tool(config, root, "skill");
            CommandOutcome::Continue
        }
        LoopCommand::ToggleAutoCopy => {
            options.auto_copy = !options.auto_copy;
            CommandOutcome::Continue
        }
        LoopCommand::ToggleClearScreen => {
            options.clear_screen = !options.clear_screen;
            CommandOutcome::Continue
        }
        LoopCommand::SkillMenu => {
            // Handled by the configuration menu loop to avoid an async
            // recursion cycle between run_command and menu loops.
            // 由配置菜单循环处理，避免 run_command 与菜单循环之间的
            // async 递归环。
            CommandOutcome::Continue
        }
        LoopCommand::ToggleContextAutoLoad => {
            config.context_auto_load = !config.context_auto_load;
            persist_and_confirm(config, root, "cli.config.saved", "");
            CommandOutcome::Continue
        }
        LoopCommand::ShowMemoryUsage => {
            let usage = session.memory_usage();
            let lines = [
                t_fmt(
                    "cli.config.memory_total",
                    &[
                        ("total", &crate::format_bytes(usage.total_bytes)),
                        ("rounds", &session.len().to_string()),
                    ],
                ),
                t_fmt(
                    "cli.config.memory_calls",
                    &[("bytes", &crate::format_bytes(usage.calls_bytes))],
                ),
                t_fmt(
                    "cli.config.memory_results",
                    &[("bytes", &crate::format_bytes(usage.results_bytes))],
                ),
                t_fmt(
                    "cli.config.memory_metadata",
                    &[("bytes", &crate::format_bytes(usage.metadata_bytes))],
                ),
            ];
            for line in lines {
                crate::console::out_println!("{}", crate::style::accent(&line));
            }
            CommandOutcome::Continue
        }
        LoopCommand::EnableAllSkills => {
            for skill in all_skills() {
                let _ = set_enabled(&skill.path, true);
            }
            CommandOutcome::Continue
        }
        LoopCommand::DisableAllSkills => {
            for skill in all_skills() {
                let _ = set_enabled(&skill.path, false);
            }
            CommandOutcome::Continue
        }
        LoopCommand::ToggleSkillAt(path) => {
            if let Some(skill) = all_skills().into_iter().find(|skill| skill.path == *path) {
                let _ = set_enabled(&skill.path, !skill.is_enabled);
            }
            CommandOutcome::Continue
        }
        LoopCommand::Back => CommandOutcome::Exit,
    }
}

/// Toggle the approval mode and print a confirmation.
/// 切换审批模式并打印确认。
pub(super) fn toggle_mode(options: &mut LoopOptions) {
    options.mode = match options.mode {
        SessionMode::Manual => SessionMode::AcceptEdit,
        SessionMode::AcceptEdit => SessionMode::Manual,
    };
    crate::console::out_println!(
        "{}",
        t_fmt(
            "cli.config.mode_switched",
            &[("mode", &mode_label(options.mode))]
        )
    );
}

/// Switch the interface language, optionally by 1-based index.
/// 切换界面语言，可选按 1-based 索引切换。
pub(super) fn switch_lang(config: &mut Config, root: &Path, index: Option<usize>) {
    const LANGS: [&str; 2] = ["en", "zh-CN"];
    if let Some(index) = index {
        if let Some(lang) = LANGS.get(index.saturating_sub(1)) {
            apply_lang(config, root, lang);
        } else {
            crate::console::out_println!(
                "{}",
                t_fmt(
                    "cli.error.invalid_index",
                    &[("count", &LANGS.len().to_string())]
                )
            );
        }
    } else {
        let lang = cycle_lang(&config.lang);
        apply_lang(config, root, &lang);
    }
}

fn apply_lang(config: &mut Config, root: &Path, lang: &str) {
    config.lang = lang.to_string();
    i18n::set_locale(&config.lang);
    persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
}

/// Switch the tool-call format, optionally by 1-based index.
/// 切换工具调用格式，可选按 1-based 索引切换。
pub(super) fn switch_format(
    registry: &FormatRegistry,
    config: &mut Config,
    root: &Path,
    index: Option<usize>,
) {
    let labels = RegistryMode::all_labels();
    if let Some(index) = index {
        if let Some(label) = labels.get(index.saturating_sub(1)) {
            apply_format(config, root, registry, label);
        } else {
            crate::console::out_println!(
                "{}",
                t_fmt(
                    "cli.error.invalid_index",
                    &[("count", &labels.len().to_string())]
                )
            );
        }
    } else {
        let label = cycle_format(&config.tool_call_format);
        apply_format(config, root, registry, &label);
    }
}

fn apply_format(config: &mut Config, root: &Path, registry: &FormatRegistry, label: &str) {
    config.tool_call_format = label.to_string();
    let _ = apply_format_mode(registry, config);
    persist_and_confirm(
        config,
        root,
        "cli.config.format_switched",
        &config.tool_call_format,
    );
}

/// Toggle one tool switch and persist the configuration.
/// 切换一个工具开关并持久化配置。
pub(super) fn toggle_tool(config: &mut Config, root: &Path, tool: &str) {
    match tool {
        "shell" => config.shell = !config.shell,
        "read" => config.read = !config.read,
        "write" => config.write = !config.write,
        "edit" => config.edit = !config.edit,
        "skill" => config.skill = !config.skill,
        _ => return,
    }
    persist_and_confirm(config, root, "cli.config.saved", "");
}

/// Persist the config and print a confirmation message.
/// 持久化配置并打印确认消息。
pub(super) fn persist_and_confirm(config: &Config, root: &Path, key: &str, value: &str) {
    match save_project(root, config) {
        Ok(()) => crate::console::out_println!(
            "{}",
            t_fmt(key, &[("lang", value), ("format", value), ("value", value)])
        ),
        Err(e) => eprintln!(
            "{}",
            t_fmt("cli.error.output", &[("error", &e.to_string())])
        ),
    }
}
