//! Configuration menu and persistence.
//! 配置菜单与持久化。

use std::io::Write;
use std::path::Path;

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::{all_skills, set_enabled};
use manualaid_ws::config::{Config, save_project};

use super::LoopOptions;
use super::utils::{apply_format_mode, cycle_format, cycle_lang, read_line, t_fmt};

/// The secondary configuration menu.
/// 二级配置菜单。
pub(super) fn config_menu(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    options: &mut LoopOptions,
) {
    loop {
        println!("{}", render_config_menu(config, options));
        let line = read_line().unwrap_or_default();
        match line.trim() {
            "1" => {
                config.lang = cycle_lang(&config.lang);
                i18n::set_locale(&config.lang);
                persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
            }
            "2" => {
                config.tool_call_format = cycle_format(&config.tool_call_format);
                let _ = apply_format_mode(registry, config);
                persist_and_confirm(
                    config,
                    root,
                    "cli.config.format_switched",
                    &config.tool_call_format,
                );
            }
            "3" => toggle_tool(config, root, "shell"),
            "4" => toggle_tool(config, root, "read"),
            "5" => toggle_tool(config, root, "write"),
            "6" => toggle_tool(config, root, "edit"),
            "7" => toggle_tool(config, root, "skill"),
            "8" => options.auto_copy = !options.auto_copy,
            "9" => options.clear_screen = !options.clear_screen,
            "10" => skill_config_menu(),
            "0" | "" => break,
            _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
        }
    }
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
        Ok(()) => println!(
            "{}",
            t_fmt(key, &[("lang", value), ("format", value), ("value", value)])
        ),
        Err(e) => eprintln!(
            "{}",
            t_fmt("cli.error.output", &[("error", &e.to_string())])
        ),
    }
}

/// Render the configuration menu with current states.
/// 渲染带当前状态的配置菜单。
pub fn render_config_menu(config: &Config, options: &LoopOptions) -> String {
    let lang_name = if config.lang == "en" {
        "English"
    } else {
        "中文"
    };
    let state = |enabled: bool| {
        if enabled {
            i18n::t_str("cli.config.enabled")
        } else {
            i18n::t_str("cli.config.disabled")
        }
    };
    [
        i18n::t_str("cli.config.title"),
        t_fmt("cli.config.lang", &[("lang", lang_name)]),
        t_fmt("cli.config.format", &[("format", &config.tool_call_format)]),
        t_fmt("cli.config.shell", &[("state", &state(config.shell))]),
        t_fmt("cli.config.read", &[("state", &state(config.read))]),
        t_fmt("cli.config.write", &[("state", &state(config.write))]),
        t_fmt("cli.config.edit", &[("state", &state(config.edit))]),
        t_fmt("cli.config.skill", &[("state", &state(config.skill))]),
        t_fmt(
            "cli.config.auto_copy",
            &[("state", &state(options.auto_copy))],
        ),
        t_fmt(
            "cli.config.clear_screen",
            &[("state", &state(options.clear_screen))],
        ),
        i18n::t_str("cli.config.skill_list"),
        i18n::t_str("cli.config.back"),
    ]
    .join("\n")
}

/// The SKILL enable/disable sub-menu: toggle by index, all on, all off.
/// SKILL 启用/禁用二级菜单：按索引切换、全部启用、全部禁用。
pub(super) fn skill_config_menu() {
    loop {
        let skills = all_skills();
        let mut lines = vec![i18n::t_str("cli.skill_config.title")];
        for (index, skill) in skills.iter().enumerate() {
            let state = if skill.is_enabled {
                i18n::t_str("cli.config.enabled")
            } else {
                i18n::t_str("cli.config.disabled")
            };
            lines.push(t_fmt(
                "cli.skill_config.item",
                &[
                    ("state", &state),
                    ("index", &(index + 1).to_string()),
                    ("name", &skill.name),
                    ("unique_name", &skill.unique_name),
                ],
            ));
        }
        println!("{}", lines.join("\n"));
        print!("{}", i18n::t_str("cli.skill_config.prompt"));
        let _ = std::io::stdout().flush();
        let line = read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if trimmed == "a" {
            for skill in &skills {
                let _ = set_enabled(&skill.path, true);
            }
            continue;
        }
        if trimmed == "n" {
            for skill in &skills {
                let _ = set_enabled(&skill.path, false);
            }
            continue;
        }
        if let Ok(index) = trimmed.parse::<usize>()
            && let Some(skill) = skills.get(index.saturating_sub(1))
        {
            let _ = set_enabled(&skill.path, !skill.is_enabled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::utils::push_test_input;
    use super::*;

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
        };
        let menu = render_config_menu(&config, &options);
        assert!(menu.contains("中文"));
        assert!(menu.contains("xml"));
    }

    #[test]
    fn toggle_tool_flips_each_tool_and_persists() {
        let root = crate::test_support::temp_dir("toggle-tools");
        for (tool, initial) in [
            ("shell", true),
            ("read", true),
            ("write", true),
            ("edit", true),
            ("skill", true),
        ] {
            let mut config = Config::default();
            toggle_tool(&mut config, &root, tool);
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
        toggle_tool(&mut config, &root, "bogus");
        assert!(!root.join(".ManualAid").join("config.toml").exists());
    }

    #[test]
    fn persist_and_confirm_reports_write_failure() {
        let root = crate::test_support::temp_dir("persist-fail");
        std::fs::write(root.join(".ManualAid"), "occupied").unwrap();
        persist_and_confirm(&Config::default(), &root, "cli.config.saved", "");
    }

    #[test]
    fn config_menu_cycles_lang_then_exits() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["1", "0"]);
        config_menu(&mut config, &registry, &root, &mut options);
        assert_eq!(config.lang, "zh-CN");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("lang = \"zh-CN\""));
    }

    #[test]
    fn config_menu_toggles_options_and_rejects_unknown_input() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-options");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["8", "9", "_", "0"]);
        config_menu(&mut config, &registry, &root, &mut options);
        assert!(!options.auto_copy);
        assert!(options.clear_screen);
    }

    #[test]
    fn config_menu_format_toggle_applies_to_registry() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let root = crate::test_support::temp_dir("config-menu-format");
        let mut config = Config::default();
        let registry = FormatRegistry::new();
        let mut options = LoopOptions::default();
        push_test_input(&["2", "0"]);
        config_menu(&mut config, &registry, &root, &mut options);
        assert_eq!(config.tool_call_format, "xml");
        assert_eq!(registry.mode().unwrap().label(), "xml");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("tool_call_format = \"xml\""));
    }

    #[test]
    fn skill_config_menu_toggles_all_and_single() {
        let _lock = crate::test_support::SKILL_LOCK.lock().unwrap();
        let root = crate::test_support::temp_dir("skill-menu-root");
        let home = crate::test_support::temp_dir("skill-menu-home");
        write_skill(&home, "alpha", "alpha");
        write_skill(&home, "beta", "beta");
        manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();
        push_test_input(&["a", "1", "n", ""]);
        skill_config_menu();
        // `set_enabled` persists to the project root config, not the home.
        let config = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(config.contains("[skill]"));
        let skills = manualaid_core::skill::all_skills();
        assert!(skills.iter().all(|skill| !skill.is_enabled));
        manualaid_core::skill::reset_skills();
    }
}
