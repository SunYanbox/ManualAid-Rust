//! System prompt building: renders the enabled-tools section with the
//! active wire format, the workspace context and the enabled-skills list,
//! plus the XML-wrapped result text copied back to the clipboard.
//! 系统提示词构建：按当前线格式渲染已启用工具区块、工作区上下文与
//! 已启用技能列表；并提供复制回剪贴板的 XML 包裹结果文本。

use std::fmt::Write;
use std::path::Path;

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::Skill;
use manualaid_core::tools::{ToolKind, ToolResult};

use crate::config::Config;

/// Build the full `<system_prompt>` text for `config`.
/// 为 `config` 构建完整的 `<system_prompt>` 文本。
pub fn build_system_prompt(
    config: &Config,
    workspace_root: &Path,
    registry: &FormatRegistry,
    skills: &[Skill],
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
    let format_desc = format!(
        "{}\n```\n{}```",
        t_fmt(
            "cli.prompt.format_desc",
            &[("format", &config.tool_call_format)]
        ),
        registry
            .render_tool_call_template(&ToolKind::Read)
            .unwrap_or_default(),
    );

    let mut out = String::new();
    out.push_str("<system_prompt>\n");
    out.push_str(&i18n::t_str("prompt.system.capabilities"));
    out.push('\n');
    out.push_str(&t_fmt(
        "prompt.system.common-rules",
        &[
            ("current_tool_format_description", &format_desc),
            ("tools_list", &tools_list),
        ],
    ));
    if skill_active {
        out.push('\n');
        out.push_str(&i18n::t_str("prompt.system.skill-rule"));
    }
    out.push('\n');

    let workspace_info = workspace_info_text(workspace_root);
    let skills_list = if skill_active {
        skills_list_text(skills)
    } else {
        String::new()
    };
    out.push_str(&t_fmt(
        "prompt.system.dynamic-context",
        &[
            ("workspace_info", &workspace_info),
            ("skills_list", &skills_list),
        ],
    ));
    out.push_str("\n</system_prompt>");
    out
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

        out.push_str("**Call template:**\n\n```\n");
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

/// The `<dynamic-context>` workspace section: root path and shell.
/// `<dynamic-context>` 的工作区部分：根路径与 Shell。
fn workspace_info_text(workspace_root: &Path) -> String {
    let shell = manualaid_core::shell::detected_shell();
    format!(
        "<workspace_root>\n{}\n</workspace_root>\n<shell_environment>\n{shell}\n</shell_environment>\n",
        workspace_root.display()
    )
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

/// Render one round's execution results as XML-wrapped text for pasting
/// back into an external LLM chat.
/// 将一轮执行结果渲染为 XML 包裹文本，供回贴到外部 LLM 聊天。
pub fn format_results(results: &[ToolResult]) -> String {
    results
        .iter()
        .map(|result| {
            let params_attr = if result.params_summary.is_empty() {
                String::new()
            } else {
                format!(" params=\"{}\"", xml_escape(&result.params_summary))
            };
            format!(
                "<tool_result name=\"{}\"{params_attr} success=\"{}\">\n{}\n</tool_result>",
                result.tool_name,
                result.success,
                result.output.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[]);
        assert!(prompt.starts_with("<system_prompt>"));
        assert!(prompt.ends_with("</system_prompt>"));
        assert!(prompt.contains("<workspace_root>"));
        assert!(prompt.contains("<shell_environment>"));
    }

    #[test]
    fn format_results_escapes_attribute_values() {
        let result = ToolResult::success("read", "content", true)
            .with_params_summary("{\"file_path\":\"/a.txt\"}".into());
        let text = format_results(&[result]);
        assert!(text.contains("<tool_result name=\"read\""));
        assert!(text.contains("&quot;file_path&quot;"));
        assert!(text.contains("success=\"true\""));
    }

    #[test]
    fn format_results_omits_empty_summary() {
        let result = ToolResult::success("shell", "done", false);
        let text = format_results(&[result]);
        assert!(text.contains("<tool_result name=\"shell\" success=\"true\">"));
        assert!(!text.contains("params="));
    }

    #[test]
    fn xml_escape_handles_all_specials() {
        assert_eq!(xml_escape("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
    }
}
