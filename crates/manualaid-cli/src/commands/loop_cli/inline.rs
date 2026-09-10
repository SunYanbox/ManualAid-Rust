//! Inline `/command` handling at the loop menu prompt.
//! 菜单提示符处内置 `/命令` 的处理。

use std::path::Path;

use manualaid_core::clipboard::{ClipboardProvider, RealClipboard};
use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::tools::ToolKind;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::LoopOptions;
use super::command;
use super::handlers::{
    copy_compressed_session_prompt_with_provider, copy_enabled_tools_with_provider,
    copy_plan_mode_rule_with_provider, copy_round_index_with_provider,
    copy_round_result_with_provider, copy_skills_list_with_provider,
    copy_switch_mode_rule_with_provider, copy_system_prompt_with_provider,
};
use super::utils::{parse_round_index, t_fmt};

/// A built-in inline command with the i18n key of its usage description.
/// 内置内联命令及其用途描述的 i18n 键。
pub(crate) struct BuiltinCommand {
    /// Primary spelling including the leading `/` (e.g. `/help`).
    /// 含起始 `/` 的主要写法（如 `/help`）。
    pub name: &'static str,
    /// Key of the localized description under `cli.cmd.*`.
    /// `cli.cmd.*` 下的本地化描述键。
    pub desc_key: &'static str,
}

/// Every built-in inline command, shared by `/help` and the completion
/// candidates; aliases remain only in `handle_inline_command`.
/// 全部内置内联命令，供 `/help` 与补全候选共用；别名仅保留在
/// `handle_inline_command` 中。
pub(crate) const BUILTIN_COMMANDS: &[BuiltinCommand] = &[
    BuiltinCommand {
        name: "/help",
        desc_key: "cli.cmd.help",
    },
    BuiltinCommand {
        name: "/history",
        desc_key: "cli.cmd.history",
    },
    BuiltinCommand {
        name: "/summary",
        desc_key: "cli.cmd.summary",
    },
    BuiltinCommand {
        name: "/clear",
        desc_key: "cli.cmd.clear",
    },
    BuiltinCommand {
        name: "/mode",
        desc_key: "cli.cmd.mode",
    },
    BuiltinCommand {
        name: "/ws",
        desc_key: "cli.cmd.ws",
    },
    BuiltinCommand {
        name: "/tools",
        desc_key: "cli.cmd.tools",
    },
    BuiltinCommand {
        name: "/skills",
        desc_key: "cli.cmd.skills",
    },
    BuiltinCommand {
        name: "/compress",
        desc_key: "cli.cmd.compress",
    },
    BuiltinCommand {
        name: "/plan",
        desc_key: "cli.cmd.plan",
    },
    BuiltinCommand {
        name: "/build",
        desc_key: "cli.cmd.build",
    },
    BuiltinCommand {
        name: "/c",
        desc_key: "cli.cmd.copy",
    },
    BuiltinCommand {
        name: "/lang",
        desc_key: "cli.cmd.lang",
    },
    BuiltinCommand {
        name: "/format",
        desc_key: "cli.cmd.format",
    },
];

/// Handle an inline `/command` typed at the menu prompt.
/// 处理在菜单提示符输入的内置 `/命令`。
pub(super) fn handle_inline_command(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    line: &str,
) {
    handle_inline_command_with_provider(
        &RealClipboard,
        config,
        registry,
        root,
        session,
        options,
        line,
    );
}

pub(super) fn handle_inline_command_with_provider<P: ClipboardProvider>(
    provider: &P,
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    line: &str,
) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }
    let cmd = parts[0];

    // Command aliases that take no arguments.
    // 无参数的命令别名。
    match cmd {
        "/help" | "/?" | "/h" => {
            crate::console::out_println!("{}", i18n::t_str("cli.help.text"));
            return;
        }
        "/history" | "/H" => {
            super::handlers::show_tool_history(session);
            return;
        }
        "/summary" | "/s" => {
            super::handlers::print_session_summary(config, session);
            return;
        }
        "/clear" | "/cls" => {
            super::utils::clear_screen();
            return;
        }
        "/mode" | "/m" => {
            command::toggle_mode(options);
            return;
        }
        "/compress" => {
            if let Err(e) = copy_compressed_session_prompt_with_provider(provider) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
            return;
        }
        "/plan" | "/p" => {
            if let Err(e) = copy_plan_mode_rule_with_provider(provider) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
            return;
        }
        "/build" | "/b" => {
            if let Err(e) = copy_switch_mode_rule_with_provider(provider) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
            return;
        }
        _ => {}
    }

    // Commands with optional positional arguments.
    // 带可选位置参数的命令。
    match parts.as_slice() {
        ["/ws"] => {
            if let Err(e) = copy_system_prompt_with_provider(provider, config, root, registry) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
        }
        ["/tools"] => {
            if let Err(e) = copy_enabled_tools_with_provider(provider, config, registry) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
        }
        ["/skills"] => {
            if let Err(e) = copy_skills_list_with_provider(provider) {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            }
        }
        ["/c"] => copy_round_result_with_provider(provider, root, session, config.max_result_chars),
        ["/c", index] => {
            if let Some(index) = parse_round_index(index, session.len()) {
                copy_round_index_with_provider(
                    provider,
                    root,
                    session,
                    index,
                    config.max_result_chars,
                );
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
                command::copy_tool_template(provider, registry, &tool);
            } else {
                crate::console::out_println!("Unknown tool `{tool_name}`");
            }
        }
        ["/lang"] => command::switch_lang(config, root, None),
        ["/lang", index] => {
            if let Ok(index) = index.parse::<usize>() {
                command::switch_lang(config, root, Some(index));
            } else {
                crate::console::out_println!(
                    "{}",
                    t_fmt("cli.error.invalid_index", &[("count", &2.to_string())])
                );
            }
        }
        ["/format"] => command::switch_format(registry, config, root, None),
        ["/format", index] => {
            if let Ok(index) = index.parse::<usize>() {
                command::switch_format(registry, config, root, Some(index));
            } else {
                crate::console::out_println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &RegistryMode::all_labels().len().to_string())]
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

    fn setup() -> (
        Config,
        FormatRegistry,
        std::path::PathBuf,
        SessionLog,
        LoopOptions,
    ) {
        let root = crate::test_support::temp_dir("inline");
        (
            Config::default(),
            FormatRegistry::new(),
            root,
            SessionLog::new(),
            LoopOptions::default(),
        )
    }

    #[test]
    fn inline_copy_tool_with_provider_writes_template() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c t read",
        );
        let clipboard = mock.read().unwrap();
        assert!(clipboard.contains("\"tool_use\": \"read\""));
    }

    #[test]
    fn inline_copy_tool_with_provider_write_error_leaves_clipboard_empty() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c t read",
        );
        assert!(mock.read().unwrap().is_empty());
    }

    fn setup_with_rounds() -> (
        Config,
        FormatRegistry,
        std::path::PathBuf,
        SessionLog,
        LoopOptions,
    ) {
        let (config, registry, root, mut session, options) = setup();
        session.push(
            vec![],
            vec![ToolResult::success("read", "file content here", true)],
            RoundStats::default(),
        );
        (config, registry, root, session, options)
    }

    #[test]
    fn inline_copy_index_with_provider_writes_round_result() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup_with_rounds();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
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
        let (mut config, registry, root, mut session, mut options) = setup_with_rounds();
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c 1",
        );
        assert!(mock.read().unwrap().is_empty());
    }

    #[test]
    fn inline_tools_copies_wrapped_full_list() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/tools",
        );
        let clipboard = mock.read().unwrap();
        assert!(clipboard.starts_with("<system-reminder>\n"));
        assert!(clipboard.ends_with("</system-reminder>"));
        assert!(clipboard.contains("## read"));
        assert!(clipboard.contains("**Parameters:**"));
    }

    #[test]
    fn inline_skills_copies_wrapped_list() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        let mock = MockClipboard::new();
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/skills",
        );
        let clipboard = mock.read().unwrap();
        assert!(clipboard.starts_with("<system-reminder>\n"));
        assert!(clipboard.ends_with("</system-reminder>"));
    }

    #[test]
    fn inline_prompt_shortcuts_copy_wrapped_rules() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        for command in ["/compress", "/plan", "/p", "/build", "/b"] {
            let (mut config, registry, root, mut session, mut options) = setup();
            let mock = MockClipboard::new();
            handle_inline_command_with_provider(
                &mock,
                &mut config,
                &registry,
                &root,
                &mut session,
                &mut options,
                command,
            );
            let clipboard = mock.read().unwrap();
            assert!(
                clipboard.contains("<system-reminder>"),
                "{command} should copy a wrapped prompt"
            );
        }
    }

    #[test]
    fn inline_lang_cycles_and_persists() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/lang",
        );
        assert_eq!(config.lang, "zh-CN");
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("lang = \"zh-CN\""));
    }

    #[test]
    fn inline_lang_with_index_and_out_of_range() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/lang 2",
        );
        assert_eq!(config.lang, "zh-CN");
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/lang 9",
        );
        assert_eq!(config.lang, "zh-CN");
    }

    #[test]
    fn inline_format_cycles_and_applies_index() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/format",
        );
        assert_eq!(config.tool_call_format, "json-codeblock");
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/format 3",
        );
        assert_eq!(config.tool_call_format, "invoke");
    }

    #[test]
    fn inline_format_out_of_range_is_rejected() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/format 9",
        );
        assert_eq!(config.tool_call_format, "auto");
    }

    #[test]
    fn inline_copy_rejects_invalid_index_and_unknown_tool() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c 9",
        );
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c t bogus",
        );
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c",
        );
    }

    #[test]
    fn inline_copy_without_rounds_prints_notice() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        // An empty session returns before any clipboard access; a valid
        // index would write to the clipboard and is not exercised.
        // 空会话在触碰剪贴板前就返回；有效索引会写入剪贴板，不做测试。
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/c",
        );
    }

    #[test]
    fn inline_unknown_command_prints_invalid() {
        let _capture = crate::console::capture();
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/xyz",
        );
    }

    #[test]
    fn inline_empty_command_does_nothing() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "",
        );
        let output = _capture.text();
        // The function returns immediately with no output; trim to ignore whitespace.
        assert!(output.trim().is_empty());
    }

    #[test]
    fn inline_help_command_prints_help() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/help",
        );
        let output = _capture.text();
        // Help text should list available commands, e.g. "/mode" is always present.
        assert!(output.contains("/mode") || output.contains("/help"));
    }

    #[test]
    fn inline_history_command_prints_history() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup_with_rounds();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/history",
        );
        let output = _capture.text();
        // History should show the tool name from the round.
        assert!(output.contains("read"));
    }

    #[test]
    fn inline_summary_command_prints_summary() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup_with_rounds();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/summary",
        );
        let output = _capture.text();
        // Summary should mention the tool used.
        assert!(output.contains("read"));
    }

    #[test]
    fn inline_clear_command_clears_screen() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let (mut config, registry, root, mut session, mut options) = setup();
        handle_inline_command(
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/clear",
        );
    }

    #[test]
    fn inline_ws_copy_write_error_leaves_clipboard_empty() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let (mut config, registry, root, mut session, mut options) = setup();
        let mock = MockClipboard::new();
        mock.set_write_error("mock write failure");

        // `/ws` prints the localized clipboard error to stderr and leaves the
        // mock clipboard empty because the write was rejected.
        // `/ws` 将本地化剪贴板错误打印到 stderr，由于写入被拒绝，mock
        // 剪贴板保持为空。
        handle_inline_command_with_provider(
            &mock,
            &mut config,
            &registry,
            &root,
            &mut session,
            &mut options,
            "/ws",
        );
        assert!(mock.read().unwrap().is_empty());
    }

    #[test]
    fn inline_copy_commands_report_write_error_and_leave_clipboard_empty() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        // Every clipboard-copying inline command shares the same write-error
        // branch: the localized error goes to stderr and the clipboard stays
        // empty because the write was rejected.
        // 所有复制到剪贴板的内联命令共用同一写失败分支：本地化错误打印到
        // stderr，由于写入被拒绝，剪贴板保持为空。
        for command in ["/compress", "/plan", "/build", "/tools", "/skills"] {
            let (mut config, registry, root, mut session, mut options) = setup();
            let mock = MockClipboard::new();
            mock.set_write_error("mock write failure");
            handle_inline_command_with_provider(
                &mock,
                &mut config,
                &registry,
                &root,
                &mut session,
                &mut options,
                command,
            );
            assert!(mock.read().unwrap().is_empty(), "{command}");
        }
    }
}
