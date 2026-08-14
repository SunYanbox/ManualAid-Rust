//! Inline `/command` handling at the loop menu prompt.
//! 菜单提示符处内置 `/命令` 的处理。

use std::path::Path;

use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::tools::ToolKind;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::config::persist_and_confirm;
use super::handlers::{
    copy_round_index_with_provider, copy_round_result_with_provider,
    copy_system_prompt_with_provider,
};
use super::utils::{
    apply_format_mode, cycle_format, cycle_lang, parse_round_index, print_muted_block, t_fmt,
};
use manualaid_core::clipboard::{ClipboardProvider, RealClipboard};

/// Handle an inline `/command` typed at the menu prompt.
/// 处理在菜单提示符输入的内置 `/命令`。
pub(super) fn handle_inline_command(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    line: &str,
) {
    handle_inline_command_with_provider(&RealClipboard, config, registry, root, session, line);
}

pub(super) fn handle_inline_command_with_provider<P: ClipboardProvider>(
    provider: &P,
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    line: &str,
) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["/ws"] => copy_system_prompt_with_provider(provider, config, root, registry),
        ["/tools"] => {
            let list = manualaid_ws::prompt::render_tools_list(config, registry);
            let _ = crate::pager::print_paged(&list);
        }
        ["/c"] => copy_round_result_with_provider(provider, session, config.max_result_chars),
        ["/c", index] => {
            if let Some(index) = parse_round_index(index, session.len()) {
                copy_round_index_with_provider(provider, session, index, config.max_result_chars);
            } else {
                crate::console::out_println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &session.len().to_string())]
                    )
                );
            }
        }
        ["/c", "t", tool_name] => {
            if let Some(tool) = ToolKind::from_name(tool_name) {
                match registry.render_tool_call_template(&tool) {
                    Ok(template) => match provider.write(&template) {
                        Ok(()) => print_muted_block(&[i18n::t_str("cli.loop.copied")]),
                        Err(e) => {
                            eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]))
                        }
                    },
                    Err(e) => eprintln!("{e}"),
                }
            } else {
                crate::console::out_println!("Unknown tool `{tool_name}`");
            }
        }
        ["/lang"] => {
            config.lang = cycle_lang(&config.lang);
            i18n::set_locale(&config.lang);
            persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
        }
        ["/lang", index] => {
            const LANGS: [&str; 2] = ["en", "zh-CN"];
            if let Ok(index) = index.parse::<usize>()
                && let Some(lang) = LANGS.get(index.saturating_sub(1))
            {
                config.lang = (*lang).to_string();
                i18n::set_locale(&config.lang);
                persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
            } else {
                crate::console::out_println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &LANGS.len().to_string())]
                    )
                );
            }
        }
        ["/format"] => {
            config.tool_call_format = cycle_format(&config.tool_call_format);
            let _ = apply_format_mode(registry, config);
            persist_and_confirm(
                config,
                root,
                "cli.config.format_switched",
                &config.tool_call_format,
            );
        }
        ["/format", index] => {
            let labels = RegistryMode::all_labels();
            if let Ok(index) = index.parse::<usize>()
                && let Some(label) = labels.get(index.saturating_sub(1))
            {
                config.tool_call_format = (*label).to_string();
                let _ = apply_format_mode(registry, config);
                persist_and_confirm(
                    config,
                    root,
                    "cli.config.format_switched",
                    &config.tool_call_format,
                );
            } else {
                crate::console::out_println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &labels.len().to_string())]
                    )
                );
            }
        }
        _ => crate::console::out_println!("{}", i18n::t_str("cli.loop.menu_invalid")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manualaid_ws::config::Config;

    fn setup() -> (Config, FormatRegistry, std::path::PathBuf, SessionLog) {
        let root = crate::test_support::temp_dir("inline");
        (
            Config::default(),
            FormatRegistry::new(),
            root,
            SessionLog::new(),
        )
    }

    #[test]
    fn inline_tools_renders_tool_list() {
        let _capture = crate::console::capture();
        let (config, registry, root, mut session) = setup();
        let mut config = config;
        handle_inline_command(&mut config, &registry, &root, &mut session, "/tools");
    }

    #[test]
    fn inline_lang_cycles_and_persists() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/lang");
        assert_eq!(config.lang, "zh-CN");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("lang = \"zh-CN\""));
    }

    #[test]
    fn inline_lang_with_index_and_out_of_range() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/lang 2");
        assert_eq!(config.lang, "zh-CN");
        handle_inline_command(&mut config, &registry, &root, &mut session, "/lang 9");
        assert_eq!(config.lang, "zh-CN");
    }

    #[test]
    fn inline_format_cycles_and_applies_index() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/format");
        assert_eq!(config.tool_call_format, "xml");
        handle_inline_command(&mut config, &registry, &root, &mut session, "/format 3");
        assert_eq!(config.tool_call_format, "json-codeblock");
    }

    #[test]
    fn inline_format_out_of_range_is_rejected() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/format 9");
        assert_eq!(config.tool_call_format, "auto");
    }

    #[test]
    fn inline_copy_rejects_invalid_index_and_unknown_tool() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/c 9");
        handle_inline_command(&mut config, &registry, &root, &mut session, "/c t bogus");
        handle_inline_command(&mut config, &registry, &root, &mut session, "/c");
    }

    #[test]
    fn inline_copy_without_rounds_prints_notice() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        // An empty session returns before any clipboard access; a valid
        // index would write to the clipboard and is not exercised.
        // 空会话在触碰剪贴板前就返回；有效索引会写入剪贴板，不做测试。
        handle_inline_command(&mut config, &registry, &root, &mut session, "/c");
    }

    #[test]
    fn inline_unknown_command_prints_invalid() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/xyz");
    }
}
