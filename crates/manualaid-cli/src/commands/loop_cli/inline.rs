//! Inline `/command` handling at the loop menu prompt.
//! 菜单提示符处内置 `/命令` 的处理。

use std::path::Path;

use manualaid_core::parser::{FormatRegistry, RegistryMode};
use manualaid_core::tools::ToolKind;
use manualaid_ws::config::Config;
use manualaid_ws::session::SessionLog;

use super::config::persist_and_confirm;
use super::handlers::{copy_round_result, copy_system_prompt};
use super::utils::{apply_format_mode, cycle_format, cycle_lang, parse_round_index, t_fmt};

/// Handle an inline `/command` typed at the menu prompt.
/// 处理在菜单提示符输入的内置 `/命令`。
pub(super) fn handle_inline_command(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    line: &str,
) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["/ws"] => copy_system_prompt(config, root, registry),
        ["/tools"] => {
            let list = manualaid_ws::prompt::render_tools_list(config, registry);
            let _ = crate::pager::print_paged(&list);
        }
        ["/c"] => copy_round_result(session, config.max_result_chars),
        ["/c", index] => {
            if let Some(index) = parse_round_index(index, session.len()) {
                let results = &session.latest(index).expect("validated index").results;
                match manualaid_core::clipboard::write_clipboard(
                    manualaid_ws::prompt::format_results(results, config.max_result_chars),
                ) {
                    Ok(()) => println!(
                        "{}",
                        t_fmt(
                            "cli.message.result_copied",
                            &[("index", &index.to_string())]
                        )
                    ),
                    Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
                }
            } else {
                println!(
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
                    Ok(template) => match manualaid_core::clipboard::write_clipboard(&template) {
                        Ok(()) => println!("{}", i18n::t_str("cli.loop.copied")),
                        Err(e) => {
                            eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]))
                        }
                    },
                    Err(e) => eprintln!("{e}"),
                }
            } else {
                println!("Unknown tool `{tool_name}`");
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
                println!(
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
                println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &labels.len().to_string())]
                    )
                );
            }
        }
        _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
    }
}
