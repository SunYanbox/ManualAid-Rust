//! Skill tool execution: loads the body of an enabled skill by its
//! agent-facing name (or full stable unique name) and returns it together
//! with a host-action JSON header.
//! Skill 工具执行：按代理可见名称（或完整稳定唯一名称）加载已启用技能的
//! 正文，连同 host-action JSON 头部一起返回。

use indexmap::IndexMap;
use serde_json::{Value, json};

use super::{ToolResult, get_string};
use crate::skill::{enabled_skills, exposed_name_map, resolve_skill};

/// Execute one skill parameter set. Only enabled skills can be loaded.
/// 执行一组 skill 参数。只有已启用的技能才能被加载。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let skill_name = get_string(params, "skill").unwrap_or_default();
    let args = get_string(params, "args").unwrap_or_default();

    match resolve_skill(&skill_name) {
        Some(skill) if skill.is_enabled => {
            // `path` uses the same `/`-separated form as the config-file skill
            // keys, so agents can join skill-relative resource paths on every
            // platform.
            // `path` 使用与配置文件技能键相同的 `/` 分隔形式，代理可在所有
            // 平台上拼接技能内的相对资源路径。
            let skill_path = skill.path.to_string_lossy().replace('\\', "/");
            let mut output = json!({
                "action": "invoke_skill",
                "skill": skill_name,
                "args": args,
                "path": skill_path,
            })
            .to_string();

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
            let enabled = enabled_skills();
            let exposed = exposed_name_map(&enabled);
            let available: Vec<&str> = enabled
                .iter()
                .map(|skill| exposed[skill.unique_name.as_str()].as_str())
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
