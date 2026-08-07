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
