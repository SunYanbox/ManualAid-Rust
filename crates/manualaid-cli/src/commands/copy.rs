//! The non-interactive `copy` subcommand: copy one of the reusable prompt
//! snippets straight to the clipboard without entering the TUI menu. It
//! reuses the same initialization steps as the interactive loop but never
//! builds an executor or a session.
//! 非交互式 `copy` 子命令：无需进入 TUI 菜单即可将某个可复用提示词片段
//! 直接复制到剪贴板。它复用交互式 loop 的初始化步骤，但不构建 executor
//! 或会话。

use std::path::{Path, PathBuf};

use manualaid_core::clipboard::{ClipboardProvider, RealClipboard};
use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::reload_skills_with_home;
use manualaid_core::user_dir::home_dir;
use manualaid_ws::context::discover_context_files;

use crate::cli::{ContextFilesSpec, CopyKind};

use super::loop_cli::{
    apply_cli_lang, apply_format_mode, copy_compressed_session_prompt_with_provider,
    copy_context_with_context_files_with_provider, copy_enabled_tools_with_provider,
    copy_intent_rule_with_provider, copy_line_ending_rule_with_provider,
    copy_plan_mode_rule_with_provider, copy_switch_mode_rule_with_provider,
    copy_system_prompt_with_context_files_with_provider, copy_task_planning_rule_with_provider,
    copy_tool_format_with_provider, format_config_issue, t_fmt,
};

/// Run the `copy` subcommand and return a localized error on failure. The
/// caller maps `Err` to a non-zero exit code; the handlers no longer print
/// clipboard errors themselves in this path.
/// 运行 `copy` 子命令，失败时返回本地化错误。调用方将 `Err` 映射为非零
/// 退出码；此路径中处理函数不再自行打印剪贴板错误。
pub fn run_copy(
    kind: CopyKind,
    lang: Option<String>,
    context_files: ContextFilesSpec,
) -> Result<(), String> {
    run_copy_with_provider(kind, lang, context_files, &RealClipboard)
}

/// Resolve the process working directory and home directory, then run the
/// copy through an injectable provider so tests can target a temporary
/// workspace and mock clipboard.
/// 解析进程工作目录与主目录，再通过可注入提供者执行复制，测试可指向临时
/// 工作区并模拟剪贴板。
fn run_copy_with_provider<P: ClipboardProvider>(
    kind: CopyKind,
    lang: Option<String>,
    context_files: ContextFilesSpec,
    provider: &P,
) -> Result<(), String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))?;
    let home = home_dir().map_err(|e| e.to_string())?;
    run_copy_at_with_provider(&current_dir, &home, kind, lang, context_files, provider)
}

/// Shared implementation for `run_copy` with an injectable clipboard
/// provider so tests can assert clipboard contents and write failures.
/// `run_copy_at` 的共享实现，剪贴板提供者可注入，便于测试断言剪贴板内容与写失败。
fn run_copy_at_with_provider<P: ClipboardProvider>(
    current_dir: &Path,
    home: &Path,
    kind: CopyKind,
    lang: Option<String>,
    context_files: ContextFilesSpec,
    provider: &P,
) -> Result<(), String> {
    // Reuse the loop's startup sequence; executor and session are not needed
    // for a read-only clipboard copy.
    // 复用 loop 的启动顺序；只读剪贴板复制不需要 executor 与 session。
    manualaid_core::manualaid_dir::ensure_project_manualaid_dir(current_dir)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    let (mut config, issues) = manualaid_ws::config::load(current_dir, home)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    apply_cli_lang(lang, &mut config);
    i18n::set_locale(&config.lang);

    for issue in &issues {
        crate::console::out_println!("{}", format_config_issue(issue));
    }

    reload_skills_with_home(current_dir, home).map_err(|e| e.to_string())?;

    let registry = FormatRegistry::new();
    apply_format_mode(&registry, &config)?;
    registry
        .set_enabled_tools(&config.enabled_tool_names())
        .map_err(|e| e.to_string())?;

    match kind {
        CopyKind::SystemPrompt => {
            let files = resolve_context_files(context_files, current_dir);
            copy_system_prompt_with_context_files_with_provider(
                provider,
                &config,
                current_dir,
                &registry,
                &files,
            )
        }
        CopyKind::Context => {
            let files = resolve_context_files(context_files, current_dir);
            copy_context_with_context_files_with_provider(provider, current_dir, &files)
        }
        CopyKind::CompressedSession => copy_compressed_session_prompt_with_provider(provider),
        CopyKind::IntentRule => copy_intent_rule_with_provider(provider),
        CopyKind::ToolFormat => copy_tool_format_with_provider(provider, &config, &registry),
        CopyKind::EnabledTools => copy_enabled_tools_with_provider(provider, &config),
        CopyKind::LineEndingRule => copy_line_ending_rule_with_provider(provider),
        CopyKind::PlanModeRule => copy_plan_mode_rule_with_provider(provider),
        CopyKind::SwitchModeRule => copy_switch_mode_rule_with_provider(provider),
        CopyKind::TaskPlanningRule => copy_task_planning_rule_with_provider(provider),
    }
}

/// Resolve a `--context-files` spec into the concrete file list to load.
/// Zero discovered files always produces an empty list for every spec.
/// 将 `--context-files` 选项解析为要加载的具体文件列表。无论哪种取值，
/// 未发现任何上下文文件时结果始终为空。
pub(crate) fn resolve_context_files(spec: ContextFilesSpec, root: &Path) -> Vec<PathBuf> {
    let files = discover_context_files(root);
    match spec {
        ContextFilesSpec::None => Vec::new(),
        ContextFilesSpec::First => files
            .into_iter()
            .next()
            .map(|file| vec![file.path])
            .unwrap_or_default(),
        ContextFilesSpec::All => files.into_iter().map(|file| file.path).collect(),
    }
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
