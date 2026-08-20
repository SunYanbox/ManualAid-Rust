//! System prompt building: renders the enabled-tools section with the
//! active wire format, the workspace context and the enabled-skills list,
//! plus the XML-wrapped result text copied back to the clipboard.
//! 系统提示词构建：按当前线格式渲染已启用工具区块、工作区上下文与
//! 已启用技能列表；并提供复制回剪贴板的 XML 包裹结果文本。

use std::fmt::Write;
use std::path::{Path, PathBuf};

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::Skill;
use manualaid_core::tools::{ToolKind, ToolResult};

use crate::config::Config;
use crate::context;

/// Build the full `<system_prompt>` text for `config`.
/// 为 `config` 构建完整的 `<system_prompt>` 文本。
pub fn build_system_prompt(
    config: &Config,
    workspace_root: &Path,
    registry: &FormatRegistry,
    skills: &[Skill],
    context_files: &[PathBuf],
) -> String {
    // With no enabled skills the skill tool is useless, so the prompt is
    // generated as if the skill switch were off: no skill rules and no
    // skill tool in the available-tools list.
    // 没有已启用技能时 Skill 工具没有用处，因此按 SKILL 开关关闭来生成
    // 提示词：不输出技能规则，也不在可用工具列表中列出 Skill 工具。
    let skill_active = config.skill && skills.iter().any(|skill| skill.is_enabled);
    let effective = Config {
        skill: skill_active,
        ..config.clone()
    };
    let tools_list = render_tools_list(&effective, registry);
    let format_desc = tool_calling_format_description(config, registry);

    let mut out = String::new();
    out.push_str("<system_prompt>\n");
    out.push_str(&i18n::t_str("prompt.system.capabilities"));
    out.push('\n');
    out.push_str(&i18n::t_str("prompt.system.system-reminder-note"));
    out.push('\n');
    out.push_str(&i18n::t_str("prompt.copy.task-planning-rule"));
    out.push('\n');
    let intent_output_rule = i18n::t_str("prompt.system.intent-output-rule");
    out.push_str(&t_fmt(
        "prompt.system.common-rules",
        &[
            ("current_tool_format_description", &format_desc),
            ("tools_list", &tools_list),
            ("intent_output_rule", &intent_output_rule),
        ],
    ));
    if skill_active {
        out.push('\n');
        out.push_str(&i18n::t_str("prompt.system.skill-rule"));
    }
    push_section(&mut out, &platform_notes_text());
    out.push('\n');

    let workspace_info = workspace_info_text(workspace_root);
    let context_files_text = if config.context_auto_load {
        context::render_context_files(context_files)
    } else {
        String::new()
    };
    let skills_list = if skill_active {
        skills_list_text(skills)
    } else {
        String::new()
    };
    out.push_str(&t_fmt(
        "prompt.system.dynamic-context",
        &[
            ("workspace_info", &workspace_info),
            ("context_files", &context_files_text),
            ("skills_list", &skills_list),
        ],
    ));
    out.push_str("\n</system_prompt>");
    out
}

/// Render the current tool-calling-format description: the localized
/// format preamble, a single default tool call template (`read`) and the
/// plain-text formatting notes. Shared by the system prompt builder and the
/// copy-prompt submenu.
/// 渲染当前工具调用格式说明：本地化的格式前言、单个默认工具调用模板
/// （`read`）与纯文本格式说明。系统提示词构建器与复制提示词二级菜单共用。
pub fn tool_calling_format_description(config: &Config, registry: &FormatRegistry) -> String {
    format!(
        "{}\n```func_calls\n{}\n```\n{}\n",
        t_fmt(
            "cli.prompt.format_desc",
            &[("format", &config.tool_call_format)]
        ),
        registry
            .render_tool_call_template(&ToolKind::Read)
            .unwrap_or_default(),
        i18n::t_str("cli.prompt.func_calls_notes")
    )
}

/// Render the `<available-tools>`-style section for the enabled tools,
/// with localized descriptions and call templates in the active format.
/// 为已启用工具渲染 `<available-tools>` 风格的区块：本地化描述与当前
/// 格式的调用模板。
pub fn render_tools_list(config: &Config, registry: &FormatRegistry) -> String {
    let mut out = String::new();
    for tool in manualaid_core::tools::all_tools() {
        if !is_enabled(config, tool) {
            continue;
        }
        let _ = write!(out, "## {}\n\n{}\n\n", tool.name(), tool.description());

        let params = tool.parameters();
        if !params.is_empty() {
            out.push_str("**Parameters:**\n");
            for param in &params {
                let requirement = if param.required {
                    "required"
                } else {
                    "optional"
                };
                let _ = writeln!(
                    out,
                    "- `{}` (`{}`) ({requirement}): {}",
                    param.name,
                    param.kind,
                    param.description()
                );
            }
            out.push('\n');
        }

        out.push_str("**Call template:**\n\n```func_calls\n");
        if let Ok(template) = registry.render_tool_call_template(tool) {
            out.push_str(&template);
        }
        out.push_str("\n```\n\n");
    }
    out
}

/// Whether `tool` is enabled by `config`.
/// `tool` 是否被 `config` 启用。
fn is_enabled(config: &Config, tool: &ToolKind) -> bool {
    match tool {
        ToolKind::Shell => config.shell,
        ToolKind::Read => config.read,
        ToolKind::Edit => config.edit,
        ToolKind::Write => config.write,
        ToolKind::Skill => config.skill,
    }
}

/// Windows-specific guidance rendered only when the prompt is built on
/// Windows; empty on other platforms.
/// 仅在 Windows 平台上构建提示词时渲染的 Windows 专属提示；其他平台为空。
fn platform_notes_text() -> String {
    platform_notes_text_for(cfg!(windows))
}

/// The Windows guidance for an explicit platform decision, split out so
/// tests can exercise both branches on any host platform.
/// 按显式平台决策返回 Windows 提示文本；拆出独立函数以便测试在任意
/// 宿主平台上显式覆盖两个分支。
fn platform_notes_text_for(is_windows: bool) -> String {
    if is_windows {
        i18n::t_str("prompt.system.platform-notes")
    } else {
        String::new()
    }
}

/// Append a blank line followed by `section` to `out` when `section` is
/// non-empty; a no-op otherwise.
/// 当 `section` 非空时，在 `out` 末尾先补一个空行再追加 `section`；
/// `section` 为空时不做任何事。
fn push_section(out: &mut String, section: &str) {
    if !section.is_empty() {
        out.push('\n');
        out.push_str(section);
    }
}

/// The `<dynamic-context>` workspace section: root path, shell, git info, and directory listing.
/// `<dynamic-context>` 的工作区部分：根路径、Shell、git 信息与目录列表。
fn workspace_info_text(workspace_root: &Path) -> String {
    let shell = manualaid_core::shell::detected_shell();
    let git_info = git_info_text(workspace_root);
    let dir_list = directory_listing_text(workspace_root);
    let mut result = format!(
        "<workspace_root>\n{}\n</workspace_root>\n<shell_environment>\n{shell}\n</shell_environment>\n",
        workspace_root.display()
    );
    if !git_info.is_empty() {
        result.push_str("<git_information>\n");
        // The git block opens with a localized snapshot note.
        // git 块开头附本地化快照备注。
        result.push_str(&i18n::t_str("prompt.system.git-status-note"));
        result.push_str("\n\n");
        result.push_str(&git_info);
        result.push_str("</git_information>\n");
    }
    if !dir_list.is_empty() {
        result.push_str("<directory_listing>\n");
        result.push_str(&dir_list);
        result.push_str("</directory_listing>\n");
    }
    result
}

/// Capture `git status` and `git log --oneline -5` output if git is available.
/// 若 git 可用，捕获 `git status` 与 `git log --oneline -5` 输出。
fn git_info_text(workspace_root: &Path) -> String {
    use std::process::Command;
    let mut output = String::new();

    // Check if git is available and the directory is a git repository.
    // 检查 git 是否可用且目录是否为 git 仓库。
    let git_check = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(workspace_root)
        .output();
    if let Ok(check) = git_check {
        if !check.status.success() {
            return String::new();
        }
    } else {
        return String::new();
    }

    // Git status
    // git 状态
    let status = Command::new("git")
        .arg("status")
        .current_dir(workspace_root)
        .output();
    if let Ok(out) = status
        && out.status.success()
        && let Ok(text) = String::from_utf8(out.stdout)
    {
        output.push_str(&text);
        if !text.ends_with('\n') {
            output.push('\n');
        }
    }

    // Git log (oneline, last 5 commits)
    // git 日志（单行，最近5条）
    let log = Command::new("git")
        .args(["log", "--oneline", "-5"])
        .current_dir(workspace_root)
        .output();
    if let Ok(out) = log
        && out.status.success()
        && let Ok(text) = String::from_utf8(out.stdout)
    {
        output.push_str(&text);
        if !text.ends_with('\n') {
            output.push('\n');
        }
    }

    output
}

/// List files and directories under the workspace root, similar to `ls -la`.
/// 列出工作区根目录下的文件和文件夹，类似 `ls -la`。
fn directory_listing_text(workspace_root: &Path) -> String {
    use std::fs;
    let mut lines = Vec::new();
    if let Ok(entries) = fs::read_dir(workspace_root) {
        let mut items: Vec<(String, bool)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            items.push((name, is_dir));
        }
        items.sort_by_key(|(name, _)| name.to_lowercase());
        for (name, is_dir) in items {
            let marker = if is_dir { "<DIR>" } else { "     " };
            lines.push(format!("{marker}  {name}"));
        }
    }
    lines.join("\n")
}

/// The `<dynamic-context>` skills section from the enabled skills.
/// 由已启用技能生成的 `<dynamic-context>` 技能部分。
fn skills_list_text(skills: &[Skill]) -> String {
    let mut out = String::new();
    for skill in skills {
        if !skill.is_enabled {
            continue;
        }
        let _ = writeln!(
            out,
            "- {} ({}): {}",
            skill.name, skill.unique_name, skill.description
        );
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("<available_skills>\n{out}</available_skills>\n")
    }
}

/// Minimum characters retained for any individual tool result that is
/// subject to truncation. Results no longer than this are never shortened.
/// 任何被截断的单个工具结果最少保留的字符数。不超过该值的结果永不被缩短。
const MIN_KEEP_CHARS: usize = 1000;

/// A tool result split into its XML wrapper parts and the variable content
/// for easy size accounting and truncation.
/// 将工具结果拆分为 XML 包裹部分与可变内容部分，便于尺寸计算与截断。
struct ResultPart {
    header: String,
    content: String,
    footer: String,
}

/// Render one round's execution results as XML-wrapped text for pasting
/// back into an external LLM chat. The character limit applies to the sum
/// of the tool outputs only (the XML wrappers are not counted). If that sum
/// exceeds `max_result_chars`, every result longer than `MIN_KEEP_CHARS`
/// is truncated proportionally to its original size, keeping at least
/// `MIN_KEEP_CHARS` characters; shorter results stay whole. Each
/// truncated result carries a notice with its removed character count, and
/// a round-level warning is appended at the end so both the user and the
/// LLM know content was omitted.
/// 将一轮执行结果渲染为 XML 包裹文本，供回贴到外部 LLM 聊天。字符限制
/// 只作用于各工具输出之和（不计 XML 包裹部分）。当该和超过
/// `max_result_chars` 时，每个超过 `MIN_KEEP_CHARS` 字符的结果按原始
/// 大小比例截断，且至少保留 `MIN_KEEP_CHARS` 字符；较短的结果保持
/// 完整。每个被截断的结果附带一条含被截断字符数的标注，末尾追加轮次
/// 警告，让用户与 LLM 都能知晓内容已被省略。
pub fn format_results(results: &[ToolResult], max_result_chars: usize) -> String {
    if results.is_empty() {
        return String::new();
    }

    let parts: Vec<ResultPart> = results
        .iter()
        .map(|result| {
            let params_attr = if result.params_summary.is_empty() {
                String::new()
            } else {
                format!(" params=\"{}\"", xml_escape(&result.params_summary))
            };
            let header = format!(
                "<tool_result name=\"{}\"{params_attr} success=\"{}\">\n",
                result.tool_name, result.success
            );
            ResultPart {
                header,
                // Keep the tool output verbatim: trimming would drop trailing
                // spaces that can be significant in read slices or code blocks.
                // 保留工具输出原文：trim 会丢失 read 切片或代码块中可能有意义的尾部空格。
                content: result.output.to_string(),
                footer: "\n</tool_result>".to_string(),
            }
        })
        .collect();

    let separator = "\n\n";
    let content_total: usize = parts.iter().map(|p| p.content.chars().count()).sum();

    if content_total <= max_result_chars {
        return parts
            .iter()
            .map(|p| format!("{}{}{}", p.header, p.content, p.footer))
            .collect::<Vec<_>>()
            .join(separator);
    }

    let round_warning = format!(
        "\n\n{}",
        i18n::t_str("truncated_round_warning")
            .replace("%{max_chars}", &max_result_chars.to_string())
            .replace("%{total_chars}", &content_total.to_string())
    );

    // Short results are never shortened and do not take part in the
    // proportional split; they still occupy their full length in the budget.
    // 短结果永不被缩短、不参与比例分配，但仍按完整长度占用预算。
    let eligible: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.content.chars().count() > MIN_KEEP_CHARS)
        .map(|(i, _)| i)
        .collect();

    // Nothing can be shortened: drop whole results from the end until the
    // remaining content fits, then append the round warning.
    // 没有可缩短的结果：从末尾整块丢弃，直到剩余内容放得下，再追加警告。
    if eligible.is_empty() {
        let mut result = String::new();
        let mut used = 0usize;
        for p in &parts {
            let content_len = p.content.chars().count();
            if used + content_len > max_result_chars {
                break;
            }
            let block = format!("{}{}{}", p.header, p.content, p.footer);
            if result.is_empty() {
                result.push_str(&block);
            } else {
                result.push_str(separator);
                result.push_str(&block);
            }
            used += content_len;
        }
        result.push_str(&round_warning);
        return result;
    }

    let eligible_orig_total: usize = eligible
        .iter()
        .map(|&i| parts[i].content.chars().count())
        .sum();
    let ineligible_total: usize = (0..parts.len())
        .filter(|i| !eligible.contains(i))
        .map(|i| parts[i].content.chars().count())
        .sum();
    let budget_for_eligible = max_result_chars.saturating_sub(ineligible_total);

    let mut allocs: Vec<usize> = vec![0; parts.len()];
    let mut raw_sum = 0usize;
    for &i in &eligible {
        let orig = parts[i].content.chars().count();
        let raw = ((budget_for_eligible as f64) * (orig as f64) / (eligible_orig_total as f64))
            .floor() as usize;
        let alloc = raw.max(MIN_KEEP_CHARS);
        allocs[i] = alloc;
        raw_sum += alloc;
    }

    // The minimum-keep floor can push the sum over the budget; take the
    // excess back from the largest allocations, never below the floor.
    // 保底下限可能使分配总和超出预算；从最大的分配开始回扣，但不低于下限。
    if raw_sum > budget_for_eligible {
        let overshoot = raw_sum - budget_for_eligible;
        let mut sorted: Vec<(usize, usize)> = eligible.iter().map(|&i| (allocs[i], i)).collect();
        sorted.sort_by_key(|(a, _)| std::cmp::Reverse(*a));
        let mut remaining = overshoot;
        for &(_alloc, idx) in &sorted {
            let can_reduce = allocs[idx].saturating_sub(MIN_KEEP_CHARS);
            let reduce = remaining.min(can_reduce);
            allocs[idx] -= reduce;
            remaining -= reduce;
            if remaining == 0 {
                break;
            }
        }
    }

    let mut blocks: Vec<String> = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        if eligible.contains(&i) {
            let orig = part.content.chars().count();
            let alloc = allocs[i];
            let notice = i18n::t_str("truncated_tool_result")
                .replace("%{original}", &orig.to_string())
                .replace("%{removed}", &(orig - alloc).to_string());
            let truncated: String = part.content.chars().take(alloc).collect();
            blocks.push(format!(
                "{}{}\n{}{}",
                part.header, truncated, notice, part.footer
            ));
        } else {
            blocks.push(format!("{}{}{}", part.header, part.content, part.footer));
        }
    }

    let mut result = blocks.join(separator);
    result.push_str(&round_warning);
    result
}

/// Escape XML special characters in an attribute value.
/// 转义 XML 属性值中的特殊字符。
fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Translate `key` and replace `%{name}` placeholders.
/// 翻译 `key` 并替换 `%{name}` 占位符。
fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tools_list_with_templates() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let list = render_tools_list(&config, &registry);
        assert!(list.contains("## read"));
        assert!(list.contains("Call template"));
        assert!(list.contains("<read>"));
        assert!(!list.contains("## shell") || config.shell);
    }

    #[test]
    fn disabled_tools_are_omitted() {
        let config = Config {
            shell: false,
            ..Config::default()
        };
        let registry = FormatRegistry::new();
        let list = render_tools_list(&config, &registry);
        assert!(!list.contains("## shell"));
    }

    #[test]
    fn system_prompt_contains_context_and_skills() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[], &[]);
        assert!(prompt.starts_with("<system_prompt>"));
        assert!(prompt.ends_with("</system_prompt>"));
        assert!(prompt.contains("<workspace_root>"));
        assert!(prompt.contains("<shell_environment>"));
    }

    #[test]
    fn system_prompt_contains_system_reminder_note_and_task_planning_rule() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[], &[]);
        assert!(prompt.contains(&i18n::t_str("prompt.system.system-reminder-note")));
        assert!(prompt.contains(&i18n::t_str("prompt.copy.task-planning-rule")));
    }

    #[test]
    fn tool_calling_format_description_contains_read_template_and_notes() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let description = tool_calling_format_description(&config, &registry);
        assert!(description.contains(&i18n::t_str("cli.prompt.func_calls_notes")));
        assert!(description.contains("<read>"));
    }

    #[test]
    fn platform_notes_text_for_covers_both_platform_decisions() {
        let windows_notes = platform_notes_text_for(true);
        assert!(windows_notes.starts_with("<platform-notes>"));
        assert!(windows_notes.contains("Windows"));
        assert!(platform_notes_text_for(false).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn platform_notes_are_non_empty_on_windows() {
        let notes = platform_notes_text();
        assert!(notes.starts_with("<platform-notes>"));
        assert!(notes.contains("Windows"));
    }

    #[cfg(not(windows))]
    #[test]
    fn platform_notes_are_empty_elsewhere() {
        assert!(platform_notes_text().is_empty());
    }

    #[test]
    fn push_section_appends_only_non_empty_sections() {
        let mut out = String::from("head");
        push_section(&mut out, "body");
        assert_eq!(out, "head\nbody");
        let mut out = String::from("head");
        push_section(&mut out, "");
        assert_eq!(out, "head");
    }

    #[test]
    fn xml_escape_handles_all_specials() {
        assert_eq!(xml_escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }

    #[test]
    fn skills_list_text_is_empty_when_nothing_is_enabled() {
        let skills = vec![Skill {
            unique_name: "greeter".into(),
            name: "greeter".into(),
            description: "says hi".into(),
            body: "## Usage\nhi".into(),
            path: std::path::PathBuf::from("/skills/greeter"),
            is_global: true,
            is_enabled: false,
        }];
        assert_eq!(skills_list_text(&skills), "");
    }
}
