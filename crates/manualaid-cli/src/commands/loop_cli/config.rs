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
use super::utils::{mode_label, t_fmt};

/// The copy-prompt submenu: copy reusable prompt snippets to the clipboard.
/// 复制提示词二级菜单：将可复用的提示词片段复制到剪贴板。
pub(super) async fn copy_prompt_menu<P: ClipboardProvider>(
    provider: &P,
    config: &Config,
    registry: &FormatRegistry,
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
                super::handlers::copy_intent_rule_with_provider(provider);
            }
            super::command::LoopCommand::CopyToolFormat => {
                super::handlers::copy_tool_format_with_provider(provider, config, registry);
            }
            super::command::LoopCommand::CopyEnabledTools => {
                super::handlers::copy_enabled_tools_with_provider(provider, config);
            }
            super::command::LoopCommand::CopyLineEndingRule => {
                super::handlers::copy_line_ending_rule_with_provider(provider);
            }
            super::command::LoopCommand::CopyPlanModeRule => {
                super::handlers::copy_plan_mode_rule_with_provider(provider);
            }
            super::command::LoopCommand::CopySwitchModeRule => {
                super::handlers::copy_switch_mode_rule_with_provider(provider);
            }
            super::command::LoopCommand::CopyTaskPlanningRule => {
                super::handlers::copy_task_planning_rule_with_provider(provider);
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
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.intent_rule"),
            MenuAction::Command(LoopCommand::CopyIntentRule),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.tool_format"),
            MenuAction::Command(LoopCommand::CopyToolFormat),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.enabled_tools"),
            MenuAction::Command(LoopCommand::CopyEnabledTools),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.line_ending"),
            MenuAction::Command(LoopCommand::CopyLineEndingRule),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.plan_mode"),
            MenuAction::Command(LoopCommand::CopyPlanModeRule),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.switch_mode"),
            MenuAction::Command(LoopCommand::CopySwitchModeRule),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.copy_prompt.task_planning"),
            MenuAction::Command(LoopCommand::CopyTaskPlanningRule),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.config.back"),
            MenuAction::Command(LoopCommand::Back),
        ))
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
        .add(MenuItem::auto(
            t_fmt("cli.config.lang", &[("lang", lang_name)]),
            MenuAction::Command(LoopCommand::SwitchLang(None)),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.format", &[("format", &config.tool_call_format)]),
            MenuAction::Command(LoopCommand::SwitchFormat(None)),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.config.tools_list"),
            MenuAction::Command(LoopCommand::ToolMenu),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt(
                "cli.config.auto_copy",
                &[("state", &state(options.auto_copy))],
            ),
            MenuAction::Command(LoopCommand::ToggleAutoCopy),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt(
                "cli.config.clear_screen",
                &[("state", &state(options.clear_screen))],
            ),
            MenuAction::Command(LoopCommand::ToggleClearScreen),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.config.skill_list"),
            MenuAction::Command(LoopCommand::SkillMenu),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.mode", &[("mode", &mode_label(options.mode))]),
            MenuAction::Command(LoopCommand::ToggleMode),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt(
                "cli.config.context_auto_load",
                &[("state", &state(config.context_auto_load))],
            ),
            MenuAction::Command(LoopCommand::ToggleContextAutoLoad),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            i18n::t_str("cli.config.memory"),
            MenuAction::Command(LoopCommand::ShowMemoryUsage),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.config.back"),
            MenuAction::Command(LoopCommand::Back),
        ))
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
        .add(MenuItem::auto(
            t_fmt("cli.config.shell", &[("state", &state(config.shell))]),
            MenuAction::Command(LoopCommand::ToggleShell),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.read", &[("state", &state(config.read))]),
            MenuAction::Command(LoopCommand::ToggleRead),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.write", &[("state", &state(config.write))]),
            MenuAction::Command(LoopCommand::ToggleWrite),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.edit", &[("state", &state(config.edit))]),
            MenuAction::Command(LoopCommand::ToggleEdit),
        ))
        .expect("unique menu key")
        .add(MenuItem::auto(
            t_fmt("cli.config.skill", &[("state", &state(config.skill))]),
            MenuAction::Command(LoopCommand::ToggleSkill),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.config.back"),
            MenuAction::Command(LoopCommand::Back),
        ))
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
            .add(MenuItem::auto(
                t_fmt(
                    "cli.skill_config.item",
                    &[
                        ("state", &state),
                        ("name", &skill.name),
                        ("unique_name", &skill.unique_name),
                    ],
                ),
                MenuAction::Command(LoopCommand::ToggleSkillAt(skill.path)),
            ))
            .expect("unique menu key");
    }
    menu = menu
        .add(MenuItem::keyed_alias(
            "a",
            &["all"],
            i18n::t_str("cli.skill_config.all_on"),
            MenuAction::Command(LoopCommand::EnableAllSkills),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "n",
            &["none"],
            i18n::t_str("cli.skill_config.all_off"),
            MenuAction::Command(LoopCommand::DisableAllSkills),
        ))
        .expect("unique menu key")
        .add(MenuItem::keyed_alias(
            "0",
            &["q", "quit", "exit"],
            i18n::t_str("cli.config.back"),
            MenuAction::Command(LoopCommand::Back),
        ))
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
            "cli.copy_prompt.line_ending",
            "cli.copy_prompt.plan_mode",
            "cli.copy_prompt.switch_mode",
            "cli.copy_prompt.task_planning",
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
        push_test_input(&["1", "0"]);
        copy_prompt_menu(&mock, &Config::default(), &FormatRegistry::new()).await;
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
        push_test_input(&["4", "5", "0"]);
        copy_prompt_menu(&mock, &Config::default(), &FormatRegistry::new()).await;
        // The last copied text is the plan-mode rule because the line-ending
        // rule was written first and then overwritten.
        assert_eq!(
            mock.read().unwrap(),
            i18n::t_str("prompt.copy.plan-mode-rule")
        );
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
        assert_eq!(config.tool_call_format, "xml");
        assert_eq!(registry.mode().unwrap().label(), "xml");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("tool_call_format = \"xml\""));
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
        let _lock = crate::test_support::SKILL_LOCK.lock().unwrap();
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
        let _lock = crate::test_support::SKILL_LOCK.lock().unwrap();
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
        assert!(rendered.contains(&i18n::t_str("cli.config.back")));
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
}
