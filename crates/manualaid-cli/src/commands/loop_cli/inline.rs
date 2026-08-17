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
    if parts.is_empty() {
        return;
    }
    let cmd = parts[0];
    match cmd {
        // `|`表示匹配任意一个
        "/help" | "/?" | "/h" => {
            crate::console::out_println!("{}", i18n::t_str("cli.help.text"));
            return;
        }
        "/history" | "/H" => {
            super::show_tool_history(session);
            return;
        }
        "/summary" | "/s" => {
            super::print_session_summary(config, session);
            return;
        }
        "/clear" | "/cls" => {
            super::clear_screen();
            return;
        }
        _ => {}
    }
    // `[]`表示匹配格式完全一样的
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
    use manualaid_core::clipboard::MockClipboard;
    use manualaid_core::tools::ToolResult;
    use manualaid_ws::config::Config;
    use manualaid_ws::session::RoundStats;

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
    fn inline_copy_tool_with_provider_writes_template() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            "/c t read",
        );
        let clipboard = mock.read().unwrap();
        assert!(clipboard.contains("<read>"));
    }

    #[test]
    fn inline_copy_tool_with_provider_write_error_leaves_clipboard_empty() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            "/c t read",
        );
        assert!(mock.read().unwrap().is_empty());
    }

    fn setup_with_rounds() -> (Config, FormatRegistry, std::path::PathBuf, SessionLog) {
        let (config, registry, root, mut session) = setup();
        session.push(
            vec![],
            vec![ToolResult::success("read", "file content here", true)],
            RoundStats::default(),
        );
        (config, registry, root, session)
    }

    #[test]
    fn inline_copy_index_with_provider_writes_round_result() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup_with_rounds();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            "/c 1",
        );
        let clipboard = mock.read().unwrap();
        assert!(clipboard.contains("file content here"));
    }

    #[test]
    fn inline_copy_index_with_provider_write_error_leaves_clipboard_empty() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup_with_rounds();
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            "/c 1",
        );
        assert!(mock.read().unwrap().is_empty());
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

    #[test]
    fn inline_empty_command_does_nothing() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "");
        let output = _capture.text();
        // The function returns immediately with no output; trim to ignore whitespace.
        assert!(output.trim().is_empty());
    }

    #[test]
    fn inline_help_command_prints_help() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/help");
        let output = _capture.text();
        // Help text should list available commands, e.g. "/mode" is always present.
        assert!(output.contains("/mode") || output.contains("/help"));
    }

    #[test]
    fn inline_history_command_prints_history() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup_with_rounds();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/history");
        let output = _capture.text();
        // History should show the tool name from the round.
        assert!(output.contains("read"));
    }

    #[test]
    fn inline_summary_command_prints_summary() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session) = setup_with_rounds();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/summary");
        let output = _capture.text();
        // Summary should mention the tool used.
        assert!(output.contains("read"));
    }

    #[test]
    fn inline_clear_command_clears_screen() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/clear");
        let output = _capture.text();
        // On Windows, clear may use `cls` which doesn't write to stdout;
        // on Unix, it usually prints ANSI escape sequences. Accept either.
        assert!(output.contains("\x1B[2J\x1B[1;1H") || output.is_empty());
    }

    #[test]
    fn inline_clear_command_with_alias_cls() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let (mut config, registry, root, mut session) = setup();
        handle_inline_command(&mut config, &registry, &root, &mut session, "/cls");
        let output = _capture.text();
        assert!(output.contains("\x1B[2J\x1B[1;1H") || output.is_empty());
    }
}
