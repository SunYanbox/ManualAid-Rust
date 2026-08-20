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
    fn render_config_menu_shows_all_states() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let menu = render_config_menu(&Config::default(), &LoopOptions::default());
        assert!(menu.contains("English"));
        assert!(menu.contains("auto"));
        assert!(menu.contains(&i18n::t_str("cli.config.enabled")));
        assert!(menu.contains(&i18n::t_str("cli.config.disabled")));
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
        push_test_input(&["8", "9", "_", "0"]);
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
        push_test_input(&["11", "0"]);
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
        push_test_input(&["12", "0"]);
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
        push_test_input(&["13", "0"]);
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
}
