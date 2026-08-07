//! Skill tool execution: loads the body of an enabled skill by its unique
//! name and returns it together with a host-action JSON header.
//! Skill 工具执行：按唯一名称加载已启用技能的正文，连同 host-action
//! JSON 头部一起返回。

use indexmap::IndexMap;
use serde_json::{Value, json};

use super::{ToolResult, get_string};
use crate::skill::{enabled_skills, get_skill};

/// Execute one skill parameter set. Only enabled skills can be loaded.
/// 执行一组 skill 参数。只有已启用的技能才能被加载。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let skill_name = get_string(params, "skill").unwrap_or_default();
    let args = get_string(params, "args").unwrap_or_default();

    let mut output = json!({
        "action": "invoke_skill",
        "skill": skill_name,
        "args": args,
    })
    .to_string();

    match get_skill(&skill_name) {
        Some(skill) if skill.is_enabled => {
            if !skill.body.trim().is_empty() {
                output.push_str("\n\n");
                output.push_str(&skill.body);
            }
            ToolResult::success("skill", output, true)
        }
        Some(_) => ToolResult::failure(
            "skill",
            format!("Skill `{skill_name}` has been disabled by the user"),
        ),
        None => {
            let available: Vec<String> = enabled_skills()
                .iter()
                .map(|skill| skill.unique_name.clone())
                .collect();
            let message = if available.is_empty() {
                format!("Skill `{skill_name}` not found — no skills are enabled")
            } else {
                format!(
                    "Skill `{skill_name}` not found. Available skills: {}.",
                    available.join(", ")
                )
            };
            ToolResult::failure("skill", message)
        }
    }
}
