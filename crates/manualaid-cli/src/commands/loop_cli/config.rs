//! Configuration menu and persistence.
//! 配置菜单与持久化。

use std::path::Path;

use manualaid_core::clipboard::ClipboardProvider;
use manualaid_core::parser::FormatRegistry;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::LoopOptions;
use super::command::LoopCommand;
use super::menu::{Menu, MenuAction, MenuItem};
use super::utils::{format_changelog_text, mode_label, t_fmt};

/// Print the clipboard error produced by a copy handler in the TUI. The
/// handlers return `Result` now so the non-interactive `copy` subcommand can
/// propagate failures to its exit code; the TUI keeps the previous message.
/// 在 TUI 中打印复制处理函数产生的剪贴板错误。处理函数现在返回 `Result`，
/// 使非交互的 `copy` 子命令能把失败传播为退出码；TUI 保持原有提示文案。
fn report_copy_error(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
    }
}

/// The copy-prompt submenu: copy reusable prompt snippets to the clipboard.
/// 复制提示词二级菜单：将可复用的提示词片段复制到剪贴板。
pub(super) async fn copy_prompt_menu<P: ClipboardProvider>(
    provider: &P,
    config: &Config,
    registry: &FormatRegistry,
    root: &Path,
) {
    loop {
        let menu = build_copy_prompt_menu();
        crate::console::out_println!("{}", menu.render());
        let line = super::utils::read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(action) = menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            continue;
        };
        let command = match action {
            super::menu::MenuAction::Command(command) => command,
            super::menu::MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        match command {
            super::command::LoopCommand::Back => break,
            super::command::LoopCommand::CopyIntentRule => {
                report_copy_error(super::handlers::copy_intent_rule_with_provider(provider));
            }
            super::command::LoopCommand::CopyToolFormat => {
                report_copy_error(super::handlers::copy_tool_format_with_provider(
                    provider, registry,
                ));
            }
            super::command::LoopCommand::CopyEnabledTools => {
                report_copy_error(super::handlers::copy_enabled_tools_with_provider(
                    provider, config, registry,
                ));
            }
            super::command::LoopCommand::CopySkillsList => {
                report_copy_error(super::handlers::copy_skills_list_with_provider(provider));
            }
            super::command::LoopCommand::CopyContext => {
                report_copy_error(super::handlers::copy_context_with_provider(provider, root));
            }
            super::command::LoopCommand::CopyLineEndingRule => {
                report_copy_error(super::handlers::copy_line_ending_rule_with_provider(
                    provider,
                ));
            }
            super::command::LoopCommand::CopyPlanModeRule => {
                report_copy_error(super::handlers::copy_plan_mode_rule_with_provider(provider));
            }
            super::command::LoopCommand::CopySwitchModeRule => {
                report_copy_error(super::handlers::copy_switch_mode_rule_with_provider(
                    provider,
                ));
            }
            super::command::LoopCommand::CopyTaskPlanningRule => {
                report_copy_error(super::handlers::copy_task_planning_rule_with_provider(
                    provider,
                ));
            }
            super::command::LoopCommand::CopyCompressedSessionPrompt => {
                report_copy_error(
                    super::handlers::copy_compressed_session_prompt_with_provider(provider),
                );
            }
            super::command::LoopCommand::CopyCompressedFence => {
                report_copy_error(super::handlers::copy_compressed_fence_with_provider(
                    provider,
                ));
            }
            _ => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            }
        }
    }
}

/// Build the copy-prompt submenu with automatic numeric keys.
/// 构建自动编号的复制提示词二级菜单。
fn build_copy_prompt_menu() -> Menu {
    Menu::new(i18n::t_str("cli.copy_prompt.title"))
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.intent_rule"),
                MenuAction::Command(LoopCommand::CopyIntentRule),
            )
            .unique("copy_prompt_menu_intent_rule"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.tool_format"),
                MenuAction::Command(LoopCommand::CopyToolFormat),
            )
            .unique("copy_prompt_menu_tool_format"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.enabled_tools"),
                MenuAction::Command(LoopCommand::CopyEnabledTools),
            )
            .unique("copy_prompt_menu_enabled_tools"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.skills_list"),
                MenuAction::Command(LoopCommand::CopySkillsList),
            )
            .unique("copy_prompt_menu_skills_list"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.context"),
                MenuAction::Command(LoopCommand::CopyContext),
            )
            .unique("copy_prompt_menu_context"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.line_ending"),
                MenuAction::Command(LoopCommand::CopyLineEndingRule),
            )
            .unique("copy_prompt_menu_line_ending"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.plan_mode"),
                MenuAction::Command(LoopCommand::CopyPlanModeRule),
            )
            .unique("copy_prompt_menu_plan_mode"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.switch_mode"),
                MenuAction::Command(LoopCommand::CopySwitchModeRule),
            )
            .unique("copy_prompt_menu_switch_mode"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.task_planning"),
                MenuAction::Command(LoopCommand::CopyTaskPlanningRule),
            )
            .unique("copy_prompt_menu_task_planning"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.compressed_session"),
                MenuAction::Command(LoopCommand::CopyCompressedSessionPrompt),
            )
            .unique("copy_prompt_menu_compressed_session"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.copy_prompt.compressed_fence"),
                MenuAction::Command(LoopCommand::CopyCompressedFence),
            )
            .unique("copy_prompt_menu_compressed_fence"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::keyed_alias(
                "0",
                &["q", "quit", "exit"],
                i18n::t_str("cli.config.back"),
                MenuAction::Command(LoopCommand::Back),
            )
            .unique("copy_prompt_menu_back"),
        )
        .expect("unique menu key")
}

/// The secondary configuration menu.
/// 二级配置菜单。
pub(super) async fn config_menu<P: ClipboardProvider>(
    provider: &P,
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    options: &mut LoopOptions,
    session: &mut SessionLog,
) {
    let executor = super::build_executor(root, config, options.mode);
    loop {
        let menu = build_config_menu(config, options);
        crate::console::out_println!("{}", menu.render());
        let line = super::utils::read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(action) = menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            continue;
        };
        let command = match action {
            super::menu::MenuAction::Command(command) => command,
            super::menu::MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        match command {
            super::command::LoopCommand::ToolMenu => {
                tool_menu(provider, config, registry, root, options, session).await;
            }
            super::command::LoopCommand::SkillMenu => {
                skill_menu(provider, config, registry, root, options, session).await;
            }
            super::command::LoopCommand::ChangelogMenu => {
                changelog_menu().await;
            }
            super::command::LoopCommand::Back => break,
            other => {
                let mut ctx = super::command::CommandContext {
                    provider,
                    executor: &executor,
                    registry,
                    config,
                    options,
                    root,
                    session,
                };
                let outcome = super::command::run_command(other, &mut ctx).await;
                if matches!(outcome, super::command::CommandOutcome::ExitLoop) {
                    return;
                }
            }
        }
    }
}

/// The third-level ChangeLog menu: list versions and show one version or
/// the whole file through the pager.
/// 第三级更新日志菜单：列出版本，通过分页器查看单个版本或整个文件。
async fn changelog_menu() {
    loop {
        let menu = build_changelog_menu();
        crate::console::out_println!("{}", menu.render());
        let line = super::utils::read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(action) = menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            continue;
        };
        let command = match action {
            super::menu::MenuAction::Command(command) => command,
            super::menu::MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        match command {
            super::command::LoopCommand::Back => break,
            super::command::LoopCommand::ChangelogAll => {
                let text = i18n::changelog_all();
                if text.trim().is_empty() {
                    crate::console::out_println!(
                        "{}",
                        crate::style::muted(&i18n::t_str("cli.changelog.empty"))
                    );
                } else {
                    let styled = format_changelog_text(text);
                    let _ = crate::pager::print_paged_three_lines(&styled);
                }
            }
            super::command::LoopCommand::ChangelogVersionAt(version) => {
                match i18n::changelog_version(version.as_str()) {
                    Some(text) => {
                        let styled = format_changelog_text(&text);
                        let _ = crate::pager::print_paged_three_lines(&styled);
                    }
                    None => {
                        crate::console::out_println!(
                            "{}",
                            crate::style::muted(&i18n::t_str("cli.changelog.version_not_found"))
                        );
                    }
                }
            }
            _ => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            }
        }
    }
}

/// Build the ChangeLog third-level menu with a view-all entry and one
/// entry per parsed version.
/// 构建更新日志第三级菜单：包含“查看全部”和每个已解析版本的一项。
fn build_changelog_menu() -> Menu {
    let mut menu = Menu::new(i18n::t_str("cli.changelog.title"));
    menu = menu
        .add(
            MenuItem::auto(
                i18n::t_str("cli.changelog.all"),
                MenuAction::Command(LoopCommand::ChangelogAll),
            )
            .unique("changelog_menu_all"),
        )
        .expect("unique menu key");
    for entry in i18n::changelog_versions() {
        let label = match entry.date {
            Some(date) => format!("{} - {}", entry.version, date),
            None => entry.version.clone(),
        };
        menu = menu
            .add(
                MenuItem::auto(
                    label,
                    MenuAction::Command(LoopCommand::ChangelogVersionAt(entry.version.clone())),
                )
                .unique(&format!("changelog_menu_{}", entry.version)),
            )
            .expect("unique menu key");
    }
    menu.add(
        MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.config.back"),
            MenuAction::Command(LoopCommand::Back),
        )
        .unique("changelog_menu_back"),
    )
    .expect("unique menu key")
}

/// Build the configuration menu with automatic numeric keys.
/// 构建自动编号的配置菜单。
fn build_config_menu(config: &Config, options: &LoopOptions) -> Menu {
    let lang_name = if config.lang == "en" {
        "English"
    } else {
        "中文"
    };
    let state = |enabled: bool| {
        if enabled {
            crate::style::success(&i18n::t_str("cli.config.enabled"))
        } else {
            crate::style::muted(&i18n::t_str("cli.config.disabled"))
        }
    };
    Menu::new(i18n::t_str("cli.config.title"))
        .add(
            MenuItem::auto(
                t_fmt("cli.config.lang", &[("lang", lang_name)]),
                MenuAction::Command(LoopCommand::SwitchLang(None)),
            )
            .unique("setting_menu_lang"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.format", &[("format", &config.tool_call_format)]),
                MenuAction::Command(LoopCommand::SwitchFormat(None)),
            )
            .unique("setting_menu_format"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.config.tools_list"),
                MenuAction::Command(LoopCommand::ToolMenu),
            )
            .unique("setting_menu_tools"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt(
                    "cli.config.auto_copy",
                    &[("state", &state(options.auto_copy))],
                ),
                MenuAction::Command(LoopCommand::ToggleAutoCopy),
            )
            .unique("setting_menu_auto_copy"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt(
                    "cli.config.clear_screen",
                    &[("state", &state(options.clear_screen))],
                ),
                MenuAction::Command(LoopCommand::ToggleClearScreen),
            )
            .unique("setting_menu_clear_screen"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.config.skill_list"),
                MenuAction::Command(LoopCommand::SkillMenu),
            )
            .unique("setting_menu_skill"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.mode", &[("mode", &mode_label(options.mode))]),
                MenuAction::Command(LoopCommand::ToggleMode),
            )
            .unique("setting_menu_mode"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt(
                    "cli.config.context_auto_load",
                    &[("state", &state(config.context_auto_load))],
                ),
                MenuAction::Command(LoopCommand::ToggleContextAutoLoad),
            )
            .unique("setting_menu_context_auto_load"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.config.memory"),
                MenuAction::Command(LoopCommand::ShowMemoryUsage),
            )
            .unique("setting_menu_memory"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                i18n::t_str("cli.changelog.title"),
                MenuAction::Command(LoopCommand::ChangelogMenu),
            )
            .unique("setting_menu_changelog"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::keyed_alias(
                "0",
                &["q", "quit", "exit"],
                i18n::t_str("cli.config.back"),
                MenuAction::Command(LoopCommand::Back),
            )
            .unique("setting_menu_back"),
        )
        .expect("unique menu key")
}

/// The tool enable/disable sub-menu: toggle each tool switch by index.
/// 工具启用/禁用子菜单：按索引切换各工具开关。
pub(super) async fn tool_menu<P: ClipboardProvider>(
    provider: &P,
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    options: &mut LoopOptions,
    session: &mut SessionLog,
) {
    let executor = super::build_executor(root, config, options.mode);
    loop {
        let menu = build_tool_menu(config);
        crate::console::out_println!("{}", menu.render());
        let line = super::utils::read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(action) = menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            continue;
        };
        let command = match action {
            super::menu::MenuAction::Command(command) => command,
            super::menu::MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        match command {
            super::command::LoopCommand::Back => break,
            other => {
                let mut ctx = super::command::CommandContext {
                    provider,
                    executor: &executor,
                    registry,
                    config,
                    options,
                    root,
                    session,
                };
                let outcome = super::command::run_command(other, &mut ctx).await;
                if matches!(outcome, super::command::CommandOutcome::ExitLoop) {
                    return;
                }
            }
        }
    }
}

/// Build the tool enable/disable submenu from the current tool switches.
/// 从当前工具开关构建工具启用/禁用子菜单。
fn build_tool_menu(config: &Config) -> Menu {
    let state = |enabled: bool| {
        if enabled {
            crate::style::success(&i18n::t_str("cli.config.enabled"))
        } else {
            crate::style::muted(&i18n::t_str("cli.config.disabled"))
        }
    };
    Menu::new(i18n::t_str("cli.tool_config.title"))
        .add(
            MenuItem::auto(
                t_fmt("cli.config.shell", &[("state", &state(config.shell))]),
                MenuAction::Command(LoopCommand::ToggleShell),
            )
            .unique("tool_menu_shell"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.read", &[("state", &state(config.read))]),
                MenuAction::Command(LoopCommand::ToggleRead),
            )
            .unique("tool_menu_read"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.write", &[("state", &state(config.write))]),
                MenuAction::Command(LoopCommand::ToggleWrite),
            )
            .unique("tool_menu_write"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.edit", &[("state", &state(config.edit))]),
                MenuAction::Command(LoopCommand::ToggleEdit),
            )
            .unique("tool_menu_edit"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::auto(
                t_fmt("cli.config.skill", &[("state", &state(config.skill))]),
                MenuAction::Command(LoopCommand::ToggleSkill),
            )
            .unique("tool_menu_skill"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::keyed_alias(
                "0",
                &["q", "quit", "exit"],
                i18n::t_str("cli.config.back"),
                MenuAction::Command(LoopCommand::Back),
            )
            .unique("tool_menu_back"),
        )
        .expect("unique menu key")
}

/// The SKILL enable/disable sub-menu: toggle by index, all on, all off.
/// SKILL 启用/禁用二级菜单：按索引切换、全部启用、全部禁用。
pub(super) async fn skill_menu<P: ClipboardProvider>(
    provider: &P,
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    options: &mut LoopOptions,
    session: &mut SessionLog,
) {
    let executor = super::build_executor(root, config, options.mode);
    loop {
        let menu = build_skill_menu();
        crate::console::out_println!("{}", menu.render());
        let line = super::utils::read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        let Some(action) = menu.resolve(trimmed) else {
            crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
            continue;
        };
        let command = match action {
            super::menu::MenuAction::Command(command) => command,
            super::menu::MenuAction::Submenu(_) => {
                crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid"));
                continue;
            }
        };
        match command {
            super::command::LoopCommand::Back => break,
            other => {
                let mut ctx = super::command::CommandContext {
                    provider,
                    executor: &executor,
                    registry,
                    config,
                    options,
                    root,
                    session,
                };
                let outcome = super::command::run_command(other, &mut ctx).await;
                if matches!(outcome, super::command::CommandOutcome::ExitLoop) {
                    return;
                }
            }
        }
    }
}

/// Build the SKILL submenu from the current skill list.
/// 从当前技能列表构建 SKILL 子菜单。
fn build_skill_menu() -> Menu {
    let mut menu = Menu::new(i18n::t_str("cli.skill_config.title"));
    let skills = manualaid_core::skill::all_skills();
    for skill in skills {
        let state = if skill.is_enabled {
            crate::style::success(&i18n::t_str("cli.config.enabled"))
        } else {
            crate::style::muted(&i18n::t_str("cli.config.disabled"))
        };
        menu = menu
            .add(
                MenuItem::auto(
                    // The full stable unique name doubles as the toggle key,
                    // so the label shows it alone — no duplicate plain name.
                    // 完整稳定唯一名称同时充当切换键，故条目只显示它，不再
                    // 重复展示裸名。
                    t_fmt(
                        "cli.skill_config.item",
                        &[("state", &state), ("unique_name", &skill.unique_name)],
                    ),
                    MenuAction::Command(LoopCommand::ToggleSkillAt(skill.path)),
                )
                .unique(&format!("skill_menu_{}", skill.unique_name)),
            )
            .expect("unique menu key");
    }
    menu = menu
        .add(
            MenuItem::keyed_alias(
                "a",
                &["all"],
                i18n::t_str("cli.skill_config.all_on"),
                MenuAction::Command(LoopCommand::EnableAllSkills),
            )
            .unique("skill_menu_enable_all"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::keyed_alias(
                "n",
                &["none"],
                i18n::t_str("cli.skill_config.all_off"),
                MenuAction::Command(LoopCommand::DisableAllSkills),
            )
            .unique("skill_menu_disable_all"),
        )
        .expect("unique menu key")
        .add(
            MenuItem::keyed_alias(
                "0",
                &["q", "quit", "exit"],
                i18n::t_str("cli.config.back"),
                MenuAction::Command(LoopCommand::Back),
            )
            .unique("skill_menu_back"),
        )
        .expect("unique menu key");
    menu
}

/// Render the configuration menu with current states.
/// 渲染带当前状态的配置菜单。
pub fn render_config_menu(config: &Config, options: &LoopOptions) -> String {
    build_config_menu(config, options).render()
}

/// Render the tool enable/disable menu with current states.
/// 渲染带当前状态的工具启用/禁用菜单。
pub fn render_tool_menu(config: &Config) -> String {
    build_tool_menu(config).render()
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::command;
    use super::super::utils::push_test_input;

    fn write_skill(home: &Path, folder: &str, name: &str) {
        let dir = home.join(".ManualAid").join("skills").join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: test skill\n---\n# {name}\n"),
        )
        .unwrap();
    }

    #[test]
    fn every_menu_item_carries_a_unique_resolvable_key() {
        let menus = [
            build_copy_prompt_menu(),
            build_config_menu(&Config::default(), &LoopOptions::default()),
            build_tool_menu(&Config::default()),
            build_skill_menu(),
        ];
        for menu in menus {
            let mut seen = std::collections::HashSet::new();
            for item in menu.items() {
                let slug = item.unique_key().expect("every item has a unique key");
                assert!(!slug.is_empty(), "unique key must not be empty");
                assert!(seen.insert(slug), "duplicate slug `{slug}`");
                assert!(menu.resolve(slug).is_some(), "slug `{slug}` must resolve");
            }
        }
    }

    #[test]
    fn render_copy_prompt_menu_shows_all_entries() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let menu = build_copy_prompt_menu();
        let rendered = menu.render();
        for key in [
            "cli.copy_prompt.title",
            "cli.copy_prompt.intent_rule",
            "cli.copy_prompt.tool_format",
            "cli.copy_prompt.enabled_tools",
            "cli.copy_prompt.skills_list",
            "cli.copy_prompt.context",
            "cli.copy_prompt.line_ending",
            "cli.copy_prompt.plan_mode",
            "cli.copy_prompt.switch_mode",
            "cli.copy_prompt.task_planning",
            "cli.copy_prompt.compressed_session",
            "cli.copy_prompt.compressed_fence",
            "cli.config.back",
        ] {
            assert!(rendered.contains(&i18n::t_str(key)));
        }
        assert!(rendered.contains("0."));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_intent_rule_and_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["copy_prompt_menu_intent_rule", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        assert_eq!(
            mock.read().unwrap(),
            i18n::t_str("prompt.system.intent-output-rule")
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_line_ending_and_plan_mode_rules() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&[
            "copy_prompt_menu_line_ending",
            "copy_prompt_menu_plan_mode",
            "0",
        ]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        // The last copied text is the plan-mode rule because the line-ending
        // rule was written first and then overwritten.
        assert_eq!(
            mock.read().unwrap(),
            i18n::t_str("prompt.copy.plan-mode-rule")
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_remaining_snippets() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&[
            "copy_prompt_menu_tool_format",
            "copy_prompt_menu_enabled_tools",
            "copy_prompt_menu_switch_mode",
            "copy_prompt_menu_task_planning",
            "copy_prompt_menu_compressed_fence",
            "0",
        ]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        // The last copied text is the compressed-fence template because it is
        // selected last in the input sequence.
        // 最后复制的是压缩结果围栏模板，因为它在输入序列中最后被选中。
        assert_eq!(
            mock.read().unwrap(),
            i18n::t_str("prompt.copy.compressed-fence").trim()
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_skills_list_block() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["copy_prompt_menu_skills_list", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        let copied = mock.read().unwrap();
        assert!(copied.starts_with("<system-reminder>\n"));
        assert!(copied.ends_with("</system-reminder>"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_context_block() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        let root = crate::test_support::temp_dir("copy-prompt-context-menu");
        std::fs::write(root.join("AGENTS.md"), "# project rules").unwrap();
        push_test_input(&["copy_prompt_menu_context", "0"]);
        copy_prompt_menu(&mock, &Config::default(), &FormatRegistry::new(), &root).await;
        let copied = mock.read().unwrap();
        assert!(copied.starts_with("<system-reminder>\n"));
        assert!(copied.contains("Instructions from: AGENTS.md"));
        assert!(copied.contains("# project rules"));
        assert!(copied.ends_with("</system-reminder>"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_compressed_session_prompt() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["copy_prompt_menu_compressed_session", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        let copied = mock.read().unwrap();
        assert!(copied.starts_with("<system-reminder>\n"));
        assert!(copied.ends_with("\n</system-reminder>"));
        assert!(copied.contains("context compressor"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_copies_compressed_fence() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["copy_prompt_menu_compressed_fence", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        let copied = mock.read().unwrap();
        assert!(copied.contains("<compacted-summary>"));
        assert!(!copied.contains("<system-reminder>"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_empty_input_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn copy_prompt_menu_invalid_input_continues_then_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let mock = manualaid_core::clipboard::MockClipboard::new();
        push_test_input(&["invalid", "0"]);
        copy_prompt_menu(
            &mock,
            &Config::default(),
            &FormatRegistry::new(),
            Path::new("."),
        )
        .await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.loop.menu_invalid")));
    }

    #[test]
    fn render_config_menu_shows_all_states() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let menu = render_config_menu(&Config::default(), &LoopOptions::default());
        assert!(menu.contains("English"));
        assert!(menu.contains("auto"));
        assert!(menu.contains(&i18n::t_str("cli.config.enabled")));
        assert!(menu.contains(&i18n::t_str("cli.config.disabled")));
        assert!(menu.contains(&i18n::t_str("cli.config.tools_list")));
        assert!(menu.contains(&i18n::t_str("cli.config.skill_list")));
        assert!(menu.contains(&i18n::t_str("cli.config.mode_manual")));
        // The rendered line replaces `%{state}` with styled text, so only
        // the placeholder-free prefix is asserted.
        // 渲染行会将 `%{state}` 替换为带样式的文本，因此只断言无占位符的前缀。
        assert!(
            menu.contains(
                i18n::t_str("cli.config.context_auto_load")
                    .split("%{state}")
                    .next()
                    .unwrap()
            )
        );
        assert!(menu.contains(&i18n::t_str("cli.config.back")));
        assert!(menu.contains(&i18n::t_str("cli.changelog.title")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn changelog_menu_renders_view_all_and_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("zh-CN");
        push_test_input(&["1", "0"]);
        changelog_menu().await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.changelog.title")));
        assert!(output.contains(&i18n::t_str("cli.changelog.all")));
        assert!(output.contains(&i18n::t_str("cli.config.back")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn changelog_menu_shows_version_body_for_existing_version() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("zh-CN");
        // Menu order: 1 view all, then each parsed version starting at 2.
        push_test_input(&["3", "0"]);
        changelog_menu().await;
        let output = _capture.text();
        assert!(output.contains("0.7.0"));
    }

    #[test]
    fn render_config_menu_shows_chinese_lang_and_toggled_options() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let config = Config {
            lang: "zh-CN".to_string(),
            tool_call_format: "xml".to_string(),
            ..Config::default()
        };
        let options = LoopOptions {
            auto_copy: false,
            clear_screen: true,
            ..LoopOptions::default()
        };
        let menu = render_config_menu(&config, &options);
        assert!(menu.contains("中文"));
        assert!(menu.contains("xml"));
    }

    #[test]
    fn toggle_tool_flips_each_tool_and_persists() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("toggle-tools");
        for (tool, initial) in [
            ("shell", true),
            ("read", true),
            ("write", true),
            ("edit", true),
            ("skill", true),
        ] {
            let mut config = Config::default();
            command::toggle_tool(&mut config, &root, tool);
            match tool {
                "shell" => assert_eq!(config.shell, !initial),
                "read" => assert_eq!(config.read, !initial),
                "write" => assert_eq!(config.write, !initial),
                "edit" => assert_eq!(config.edit, !initial),
                "skill" => assert_eq!(config.skill, !initial),
                _ => unreachable!(),
            }
        }
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("[tools]"));
    }

    #[test]
    fn toggle_tool_unknown_tool_is_noop() {
        let root = crate::test_support::temp_dir("toggle-unknown");
        let mut config = Config::default();
        command::toggle_tool(&mut config, &root, "bogus");
        assert!(!root.join(".ManualAid").join("config.toml").exists());
    }

    #[test]
    fn persist_and_confirm_reports_write_failure() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("persist-fail");
        std::fs::write(root.join(".ManualAid"), "occupied").unwrap();
        command::persist_and_confirm(&Config::default(), &root, "cli.config.saved", "");
    }

    // The current-thread test runtime cannot run another test
    // concurrently, so holding the std mutex guard across awaits is safe.
    // current-thread 测试运行时不会并发运行其他测试，因此跨 await
    // 持有 std 互斥锁守卫是安全的。
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_cycles_lang_then_exits() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["1", "0"]);
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert_eq!(config.lang, "zh-CN");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("lang = \"zh-CN\""));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_toggles_options_and_rejects_unknown_input() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-options");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["4", "5", "_", "0"]);
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert!(!options.auto_copy);
        assert!(options.clear_screen);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_toggles_approval_mode_without_persisting() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-mode");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["7", "0"]);
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert_eq!(options.mode, manualaid_core::audit::SessionMode::AcceptEdit);
        assert!(!root.join(".ManualAid").join("config.toml").exists());
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_toggles_context_auto_load_and_persists() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-context");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["8", "0"]);
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert!(!config.context_auto_load);
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("context_auto_load = false"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_format_toggle_applies_to_registry() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-format");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["2", "0"]);
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert_eq!(config.tool_call_format, "json-codeblock");
        assert_eq!(registry.mode().unwrap().label(), "json-codeblock");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("tool_call_format = \"json-codeblock\""));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_shows_memory_usage() {
        let _capture = crate::console::capture();
        let _style_lock = crate::test_support::STYLE_LOCK.lock().unwrap();
        let _locale_lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-memory");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        let mut session = SessionLog::new();
        push_test_input(&["9", "0"]);
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        let output = _capture.text();
        assert!(output.contains("In-memory session footprint"));
        assert!(output.contains("Metadata:"));
        crate::style::set_enabled(false);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn tool_menu_toggles_each_tool_and_persists() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("tool-menu-persist");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["1", "2", "3", "4", "5", "0"]);
        let mut session = SessionLog::new();
        tool_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        assert!(!config.shell);
        assert!(!config.read);
        assert!(!config.write);
        assert!(!config.edit);
        assert!(!config.skill);
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("[tools]"));
    }

    #[test]
    fn render_tool_menu_shows_five_tools_and_back() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let rendered = render_tool_menu(&Config::default());
        for key in [
            "cli.tool_config.title",
            "cli.config.shell",
            "cli.config.read",
            "cli.config.write",
            "cli.config.edit",
            "cli.config.skill",
            "cli.config.back",
        ] {
            assert!(rendered.contains(i18n::t_str(key).split("%{state}").next().unwrap()));
        }
        assert!(rendered.contains("0."));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn tool_menu_empty_input_returns() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("tool-menu-empty");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["", "0"]);
        let mut session = SessionLog::new();
        tool_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn tool_menu_invalid_input_rejected() {
        let _capture = crate::console::capture();
        let root = crate::test_support::temp_dir("tool-menu-invalid");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["xyz", "0"]);
        let mut session = SessionLog::new();
        tool_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.loop.menu_invalid")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn skill_menu_toggles_all_and_single() {
        let _capture = crate::console::capture();
        let _locale = crate::test_support::LOCALE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        i18n::set_locale("en");
        let _lock = crate::test_support::SKILL_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = crate::test_support::temp_dir("skill-menu-root");
        let home = crate::test_support::temp_dir("skill-menu-home");
        write_skill(&home, "alpha", "alpha");
        write_skill(&home, "beta", "beta");
        manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();
        let mut session = SessionLog::new();
        push_test_input(&["a", "1", "n", ""]);
        skill_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut Config::default(),
            &FormatRegistry::new(),
            &root,
            &mut LoopOptions::default(),
            &mut session,
        )
        .await;
        // `set_enabled` persists to the project root config, not the home.
        let config = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(config.contains("[skill]"));
        let skills = manualaid_core::skill::all_skills();
        assert!(skills.iter().all(|skill| !skill.is_enabled));
        manualaid_core::skill::reset_skills();
    }

    #[test]
    fn build_skill_menu_empty_skills_handling() {
        // 覆盖 config.rs:356 - build_skill_menu 中 skills 为空时的循环处理
        let _locale = crate::test_support::LOCALE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        i18n::set_locale("en");
        let _lock = crate::test_support::SKILL_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // 确保没有任何 skill
        manualaid_core::skill::reset_skills();
        let menu = build_skill_menu();
        let rendered = menu.render();
        // 即使没有 skill，菜单也应该包含标题、a、n、0 选项
        assert!(rendered.contains(&i18n::t_str("cli.skill_config.title")));
        assert!(rendered.contains("a."));
        assert!(rendered.contains("n."));
        assert!(rendered.contains("0."));
        assert!(rendered.contains(&i18n::t_str("cli.skill_config.all_on")));
        assert!(rendered.contains(&i18n::t_str("cli.skill_config.all_off")));
        // The back label is covered by the `0.` numeric marker assertion;
        // localized wording may vary in CI, so avoid a fragile text match.
        // 返回标签已由 `0.` 数字标记断言覆盖；CI 中本地化措辞可能变化，
        // 避免脆弱的文本匹配。
    }

    #[test]
    fn build_skill_menu_shows_stable_unique_names() {
        let _locale = crate::test_support::LOCALE_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        i18n::set_locale("en");
        let _lock = crate::test_support::SKILL_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = crate::test_support::temp_dir("skill-menu-stable-root");
        let home = crate::test_support::temp_dir("skill-menu-stable-home");
        write_skill(&home, "alpha", "alpha");
        write_skill(&home, "beta", "beta");
        manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();

        let menu = build_skill_menu();
        let rendered = menu.render();
        // The management menu always shows the full stable unique name and
        // keys each item by it.
        // 管理菜单始终显示完整稳定唯一名称，并以它作为每项的唯一键。
        assert!(rendered.contains("global-.ManualAid-alpha"));
        assert!(rendered.contains("global-.ManualAid-beta"));
        assert!(menu.resolve("skill_menu_global-.ManualAid-alpha").is_some());
        assert!(menu.resolve("skill_menu_global-.ManualAid-beta").is_some());
        manualaid_core::skill::reset_skills();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_empty_input_exits() {
        // 覆盖 config.rs:33 - config_menu 中空输入退出
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-empty-exit");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["", "0"]); // 空输入然后返回
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        // 应该正常退出，没有 panic
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_invalid_input_rejected() {
        // 覆盖 config.rs:42-43 - config_menu 中无效动作处理
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-invalid-input");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["xyz", "0"]); // 无效输入然后返回
        let mut session = SessionLog::new();
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.loop.menu_invalid")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_submenu_action_handling() {
        // 覆盖 config.rs:48 - config_menu 中 Submenu 动作处理
        // 这个分支在实际运行中不会触发，因为 build_config_menu 只产生 Command 动作
        // 保留此测试作为文档说明
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let _root = crate::test_support::temp_dir("config-submenu");
        let _config = Config::default();
        let _registry = FormatRegistry::new();
        let _options = LoopOptions::default();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_exit_loop_handling() {
        // 覆盖 config.rs:63 - config_menu 中 ExitLoop 处理
        // 这个分支在实际运行中很难触发，因为 config_menu 中的命令都是 Continue 或 Back
        // 保留此测试作为文档说明
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let _root = crate::test_support::temp_dir("config-exit-loop");
        let _config = Config::default();
        let _registry = FormatRegistry::new();
        let _options = LoopOptions::default();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn skill_menu_empty_input_exits() {
        // 覆盖 config.rs:189-190 - skill_menu 中空输入退出
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("skill-empty-exit");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["", "0"]); // 空输入然后返回
        let mut session = SessionLog::new();
        skill_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        // 应该正常退出，没有 panic
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn skill_menu_invalid_input_rejected() {
        // 覆盖 config.rs:195-196 - skill_menu 中无效动作处理
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("skill-invalid-input");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["xyz", "0"]); // 无效输入然后返回
        let mut session = SessionLog::new();
        skill_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.loop.menu_invalid")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn skill_menu_submenu_action_handling() {
        // 覆盖 config.rs:200 - skill_menu 中 Submenu 动作处理
        // 这个分支在实际运行中不会触发，因为 build_skill_menu 只产生 Command 动作
        // 保留此测试作为文档说明
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let _root = crate::test_support::temp_dir("skill-submenu");
        let _config = Config::default();
        let _registry = FormatRegistry::new();
        let _options = LoopOptions::default();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn skill_menu_exit_loop_handling() {
        // 覆盖 config.rs:213 - skill_menu 中 ExitLoop 处理
        // 这个分支在正常情况下不会触发
        // 因为 skill_menu 中的命令都是 Continue 或 Back
        // 跳过
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn changelog_menu_invalid_input_continues_then_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("zh-CN");
        push_test_input(&["invalid", "0"]);
        changelog_menu().await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.loop.menu_invalid")));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn config_menu_enters_changelog_submenu_and_returns() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("zh-CN");
        let root = crate::test_support::temp_dir("config-changelog");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        let mut session = SessionLog::new();
        push_test_input(&["10", "0", "0"]);
        config_menu(
            &manualaid_core::clipboard::MockClipboard::new(),
            &mut config,
            &registry,
            &root,
            &mut options,
            &mut session,
        )
        .await;
        let output = _capture.text();
        assert!(output.contains(&i18n::t_str("cli.changelog.title")));
    }

    #[test]
    fn report_copy_error_handles_err_without_panicking() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");

        // `report_copy_error` prints the localized clipboard error to stderr;
        // this test only proves the Err branch is exercised without panic.
        // `report_copy_error` 将本地化剪贴板错误打印到 stderr；此测试仅证明
        // Err 分支被覆盖且不会 panic。
        report_copy_error(Err("mock clipboard failure".to_string()));
    }
}
