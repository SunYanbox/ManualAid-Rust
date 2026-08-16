//! The `debug whitelist` command: inspect the audit command whitelist at
//! three layers — built-in defaults, per-source config (project / global),
//! the merged effective list, and whether any entry conflicts with the
//! blacklist.
//! `debug whitelist` 命令：分层检查审计命令白名单——内置默认值、各来源
//! 配置（项目/全局）、合并后的生效列表，以及是否存在黑名单冲突。

use std::path::Path;

use manualaid_core::audit::{
    default_allowed_commands, is_dangerous_allow_command, sanitize_allow_commands,
};
use manualaid_ws::config::ConfigFile;

use crate::env::{current_dir, home_dir};
use crate::style;
use crate::{pager, t_fmt};

/// Target visible width for wrapping command entries (excludes ANSI codes).
/// 命令条目换行的目标可见宽度（不含 ANSI 控制码）。
const WRAP_WIDTH: usize = 76;

/// Show the audit whitelist across all layers.
/// 查看各层审计白名单。
pub fn run_whitelist(home: Option<&Path>, project: Option<&Path>) -> Result<(), String> {
    let home = match home {
        Some(h) => h.to_path_buf(),
        None => home_dir()?,
    };
    let project_root = match project {
        Some(p) => p.to_path_buf(),
        None => current_dir()?,
    };

    let default_list = default_allowed_commands();
    let project_config = read_project_config(&project_root)?;
    let global_config = read_global_config(&home)?;
    let (effective, issues) = merge_allow_commands(
        project_config.permissions.allow_commands.clone(),
        global_config.permissions.allow_commands.clone(),
    );

    let mut lines = Vec::new();

    // Default whitelist.
    // 内置默认白名单。
    lines.push(header_line(
        "cli.debug.whitelist_default",
        default_list.len(),
    ));
    lines.extend(format_command_group("  ", &default_list));
    lines.push(String::new());

    // Per-source config lists.
    // 各来源配置列表。
    let project_cmds = project_config
        .permissions
        .allow_commands
        .as_deref()
        .unwrap_or(&[]);
    lines.push(header_line(
        "cli.debug.whitelist_project",
        project_cmds.len(),
    ));
    if project_cmds.is_empty() {
        lines.push(indent_muted(
            t_fmt("cli.debug.whitelist_empty", &[]).as_str(),
        ));
    } else {
        lines.extend(format_command_group("  ", project_cmds));
    }
    lines.push(String::new());

    let global_cmds = global_config
        .permissions
        .allow_commands
        .as_deref()
        .unwrap_or(&[]);
    lines.push(header_line("cli.debug.whitelist_global", global_cmds.len()));
    if global_cmds.is_empty() {
        lines.push(indent_muted(
            t_fmt("cli.debug.whitelist_empty", &[]).as_str(),
        ));
    } else {
        lines.extend(format_command_group("  ", global_cmds));
    }
    lines.push(String::new());

    // Effective (merged) whitelist.
    // 合并后生效的白名单。
    lines.push(header_line(
        "cli.debug.whitelist_effective",
        effective.len(),
    ));
    lines.extend(if effective.is_empty() {
        vec![t_fmt("cli.debug.whitelist_empty", &[])]
    } else {
        format_command_group("  ", &effective)
    });
    lines.push(String::new());

    // Blacklist conflict detection.
    // 黑名单冲突检测。
    lines.push(header_line("cli.debug.whitelist_blacklist_check", 0));
    let conflicts: Vec<&str> = effective
        .iter()
        .map(|s| s.as_str())
        .filter(|cmd| is_dangerous_allow_command(cmd))
        .collect();
    if conflicts.is_empty() {
        lines.push(indent_muted(
            t_fmt("cli.debug.whitelist_blacklist_clean", &[]).as_str(),
        ));
    } else {
        for cmd in &conflicts {
            lines.push(indent_error(&format_command_item(cmd)));
        }
    }
    if !issues.is_empty() {
        lines.push(String::new());
        lines.push(header_line("cli.debug.whitelist_ignored", issues.len()));
        for issue in &issues {
            lines.push(indent(issue));
        }
    }

    pager::print_paged(&lines.join("\n"))
        .map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Format a group of command strings as one or more wrapped lines.
/// Each command is rendered with styling inside literal double quotes.
/// 将一组命令格式化为一个或多个换行后的行，每行最多包含可显示宽度内的多个
/// 命令；每个命令以样式化文本加字面双引号呈现。
fn format_command_group(indent_str: &str, cmds: &[String]) -> Vec<String> {
    assert!(
        !cmds.is_empty(),
        "format_command_group called with empty cmds"
    );
    let indent_visible_len = style::strip_ansi(indent_str).len();
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_visible_len = 0usize;
    for cmd in cmds {
        let item = format_command_item(cmd);
        let item_visible_len = style::strip_ansi(&item).len();
        if current.is_empty() {
            current.push_str(indent_str);
            current.push_str(&item);
            current.push(' ');
            current_visible_len = indent_visible_len + item_visible_len + 1;
        } else if current_visible_len + item_visible_len + 1 > WRAP_WIDTH {
            current.pop();
            lines.push(current);
            current = String::from(indent_str);
            current.push_str(&item);
            current.push(' ');
            current_visible_len = indent_visible_len + item_visible_len + 1;
        } else {
            current.push(' ');
            current.push_str(&item);
            current.push(' ');
            current_visible_len += item_visible_len + 1;
        }
    }
    if !current.is_empty() {
        current.pop(); // remove trailing space
        lines.push(current);
    }
    lines
}

/// Render one command as styled text inside literal unstyled double quotes.
/// 将单个命令渲染为带字面双引号的样式化文本。
fn format_command_item(cmd: &str) -> String {
    format!(r#""{}""#, style::bold(cmd))
}

/// Read the project config file from `<project_root>/.ManualAid/config.toml`;
/// returns an empty config when missing.
/// 从 `<project_root>/.ManualAid/config.toml` 读取项目配置；文件缺失时
/// 返回空配置。
fn read_project_config(root: &Path) -> Result<ConfigFile, String> {
    let path = root.join(".ManualAid").join("config.toml");
    read_config_file(&path)
}

/// Read the global config file from `<home>/.ManualAid/config.toml`;
/// returns an empty config when missing.
/// 从 `<home>/.ManualAid/config.toml` 读取全局配置；文件缺失时返回空配置。
fn read_global_config(home: &Path) -> Result<ConfigFile, String> {
    let path = home.join(".ManualAid").join("config.toml");
    read_config_file(&path)
}

/// Read and parse one config.toml; a missing file yields the default
/// (empty) config.
/// 读取并解析一个 config.toml；文件缺失时返回默认（空）配置。
fn read_config_file(path: &Path) -> Result<ConfigFile, String> {
    manualaid_ws::config::read_config_file_at(path).map_err(|e| e.to_string())
}

/// Merge project and global allow_commands (project wins) and run the
/// blacklist sanitizer; returns the effective list and any ignored-dangerous
/// command messages.
/// 合并项目与全局 allow_commands（项目优先）并运行黑名单过滤；
/// 返回生效列表与被忽略的危险命令消息。
fn merge_allow_commands(
    project: Option<Vec<String>>,
    global: Option<Vec<String>>,
) -> (Vec<String>, Vec<String>) {
    let source = match (&project, &global) {
        (Some(cmds), _) if !cmds.is_empty() => cmds,
        (_, Some(cmds)) if !cmds.is_empty() => cmds,
        _ => &[][..],
    };
    let (kept, ignored) = sanitize_allow_commands(source.to_vec());
    let ignored_msgs = ignored
        .into_iter()
        .map(|cmd| format!("`{cmd}` matched a blacklisted command and was ignored"))
        .collect();
    (kept, ignored_msgs)
}

fn header_line(key: &str, count: usize) -> String {
    t_fmt(key, &[("count", &count.to_string())])
}

fn indent(s: &str) -> String {
    format!("  {s}")
}

fn indent_muted(s: &str) -> String {
    style::muted(&format!("  {s}"))
}

fn indent_error(s: &str) -> String {
    style::error(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::acquire_locale_lock;

    #[test]
    fn merge_prefers_project_over_global() {
        let (effective, _ignored) = merge_allow_commands(
            Some(vec!["git log *".to_string()]),
            Some(vec!["cargo fmt".to_string()]),
        );
        assert_eq!(effective, vec!["git log *"]);
    }

    #[test]
    fn merge_falls_back_to_global_when_project_empty() {
        let (effective, _ignored) =
            merge_allow_commands(Some(vec![]), Some(vec!["cargo check".to_string()]));
        assert_eq!(effective, vec!["cargo check"]);
    }

    #[test]
    fn merge_falls_back_to_global_when_project_none() {
        let (effective, _ignored) =
            merge_allow_commands(None, Some(vec!["cargo check".to_string()]));
        assert_eq!(effective, vec!["cargo check"]);
    }

    #[test]
    fn merge_returns_empty_when_both_none() {
        let (effective, _ignored) = merge_allow_commands(None, None);
        assert!(effective.is_empty());
    }

    #[test]
    fn merge_ignores_dangerous_entries() {
        let (effective, ignored) = merge_allow_commands(
            Some(vec![
                "git status".to_string(),
                "rm *".to_string(),
                "cargo fmt".to_string(),
            ]),
            None,
        );
        assert_eq!(effective, vec!["git status", "cargo fmt"]);
        assert_eq!(ignored.len(), 1);
        assert!(ignored[0].contains("rm *"));
    }

    #[test]
    fn default_whitelist_contains_expected_entries() {
        let defaults = default_allowed_commands();
        assert!(defaults.contains(&"ls *".to_string()));
        assert!(defaults.contains(&"git status*".to_string()));
        assert!(defaults.contains(&"cargo check*".to_string()));
        assert!(defaults.contains(&"gh pr view*".to_string()));
    }

    #[test]
    fn is_dangerous_allow_command_detects_known_patterns() {
        assert!(is_dangerous_allow_command("*"));
        assert!(is_dangerous_allow_command("rm *"));
        assert!(!is_dangerous_allow_command("git log *"));
        assert!(!is_dangerous_allow_command("cargo fmt -- --check"));
    }

    #[test]
    fn format_command_item_wraps_in_quotes() {
        let item = format_command_item("git status");
        // Plain mode (no ANSI): plain text with literal quotes.
        assert_eq!(item, r#""git status""#);
    }

    #[test]
    fn format_command_group_wraps_multiple_items() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let lines = format_command_group("", &items);
        assert_eq!(lines.len(), 1, "expected 1 line, got {}", lines.len());
        assert!(lines[0].contains("\"a\""));
        assert!(lines[0].contains("\"c\""));
    }

    #[test]
    fn format_command_group_splits_long_lines() {
        // When wrapped items exceed WRAP_WIDTH, a second line is produced.
        // "  " + many long items must not fit on one line.
        let long_items: Vec<String> = (0..20)
            .map(|i| format!("command-with-very-long-name-{i}"))
            .collect();
        let lines = format_command_group("", &long_items);
        assert!(
            lines.len() > 1,
            "expected multi-line output for long items, got {} line(s): {:?}",
            lines.len(),
            lines
        );
    }

    #[tokio::test]
    async fn run_whitelist_prints_sections_without_config() {
        let _capture = crate::console::capture();
        let _lang = acquire_locale_lock();
        i18n::set_locale("en");
        let dir = crate::test_support::temp_dir("whitelist-no-config");
        let home = crate::test_support::temp_dir("whitelist-no-config-home");
        // Ensure .ManualAid dirs exist so config reads don't fail.
        std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
        std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
        let result = run_whitelist(Some(&home), Some(&dir));
        assert!(result.is_ok(), "unexpected error: {result:?}");
        let text = _capture.text();
        assert!(text.contains("Default whitelist"));
        assert!(text.contains("Project whitelist"));
        assert!(text.contains("Global whitelist"));
        assert!(text.contains("Effective whitelist"));
        assert!(text.contains("Blacklist conflict check"));
    }

    #[tokio::test]
    async fn run_whitelist_shows_config_entries() {
        let _capture = crate::console::capture();
        let _lang = acquire_locale_lock();
        i18n::set_locale("en");
        let dir = crate::test_support::temp_dir("whitelist-with-config");
        let home = crate::test_support::temp_dir("whitelist-with-config-home");
        std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
        std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
        std::fs::write(
            dir.join(".ManualAid").join("config.toml"),
            "[permissions]\nallow_commands = [\"gh issue list *\", \"cargo test *\"]\n",
        )
        .unwrap();
        let result = run_whitelist(Some(&home), Some(&dir));
        assert!(result.is_ok(), "unexpected error: {result:?}");
        let text = _capture.text();
        assert!(
            text.contains(r#""gh issue list *""#),
            "text does not contain expected command:\n{text}"
        );
        assert!(
            text.contains(r#""cargo test *""#),
            "text does not contain expected command:\n{text}"
        );
    }

    #[tokio::test]
    async fn run_whitelist_detects_dangerous_config_entries() {
        let _capture = crate::console::capture();
        let _lang = acquire_locale_lock();
        i18n::set_locale("en");
        let dir = crate::test_support::temp_dir("whitelist-dangerous");
        let home = crate::test_support::temp_dir("whitelist-dangerous-home");
        std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
        std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
        std::fs::write(
            dir.join(".ManualAid").join("config.toml"),
            "[permissions]\nallow_commands = [\"rm *\", \"git status\"]\n",
        )
        .unwrap();
        let result = run_whitelist(Some(&home), Some(&dir));
        assert!(result.is_ok(), "unexpected error: {result:?}");
        let text = _capture.text();
        // `rm *` appears in the raw project config but is excluded from
        // the effective list; the ignored section explains why.
        // `rm *` 会出现在原始项目配置中，但会被排除在生效列表外；
        // 忽略段会说明原因。
        assert!(text.contains("Project whitelist"));
        assert!(text.contains(r#""git status""#));
        assert!(text.contains("ignored"));
    }
}
