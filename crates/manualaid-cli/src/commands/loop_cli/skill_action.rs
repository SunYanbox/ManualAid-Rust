//! `/SKILL`-prefixed skill-loading commands typed in the main menu input
//! box. Resolved skills run through the skill tool; disabled skills print a
//! localized hint, and unknown names fall through to `handle_inline_command`.
//! 在主菜单输入框中输入的以 `/SKILL` 开头的技能加载命令。已解析的技能经
//! skill 工具运行；已禁用的技能打印本地化提示；未知名回退到
//! `handle_inline_command`。

use indexmap::IndexMap;
use serde_json::Value;
use std::path::Path;

use manualaid_core::clipboard::ClipboardProvider;
use manualaid_core::executor::Executor;
use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::{ToolCallFormat, UserActionKind};
use manualaid_ws::config::Config;
use manualaid_ws::session::{RoundStats, SessionLog};

use super::LoopOptions;
use super::handlers::finish_round_with_provider;
use super::utils::t_fmt;

/// Run a `/SKILL` line when its first token resolves to a loaded skill.
/// Returns `true` when the line was handled as a skill (enabled or disabled);
/// returns `false` when no skill matches so the caller can fall through to
/// the existing inline command dispatch.
/// 当首个 token 解析为已加载技能时运行 `/SKILL` 行。该行被作为技能处理
/// （启用或禁用）时返回 `true`；无技能匹配时返回 `false`，调用方可回退到
/// 既有的内联命令分发。
pub(super) async fn run_skill_action<P: ClipboardProvider>(
    provider: &P,
    executor: &Executor,
    root: &Path,
    config: &mut Config,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    line: &str,
) -> bool {
    let body = line.trim_start_matches('/');
    let skill_name = body.split_whitespace().next().unwrap_or_default();
    if skill_name.is_empty() {
        return false;
    }

    match manualaid_core::skill::resolve_skill(skill_name) {
        Some(skill) if skill.is_enabled => {
            let args = body[skill_name.len()..].trim();
            let exposed = manualaid_core::skill::exposed_name_map(std::slice::from_ref(&skill));
            // The USER_ACTION label must be the exposed name, matching the
            // name the external LLM sees in the skill tool invocation.
            // USER_ACTION 的 label 必须是暴露名，与外部 LLM 在 skill 工具
            // 调用中看到的名字一致。
            let label = exposed
                .values()
                .next()
                .cloned()
                .unwrap_or_else(|| skill.name.clone());
            let call = build_skill_call(skill_name, args);
            let round_start = std::time::Instant::now();
            let mut result = executor.execute(call.clone()).await;
            result.audit_decisions.clear();
            let result = result.with_user_action(UserActionKind::Skill, label);
            let stats = RoundStats {
                parse_duration_ms: 0,
                audit_duration_ms: 0,
                total_execution_duration_ms: result.execution_duration_ms,
                total_tokens: result.estimated_tokens,
            };
            finish_round_with_provider(
                provider,
                root,
                session,
                options,
                config.max_result_chars,
                round_start,
                vec![call],
                vec![result],
                stats,
            )
            .await;
            true
        }
        Some(_) => {
            crate::console::out_println!(
                "{}",
                t_fmt("cli.complete.skill_disabled", &[("skill", skill_name)])
            );
            true
        }
        None => false,
    }
}

/// Build the skill `ParsedToolCall` for a `/SKILL args...` line.
/// 为 `/SKILL args...` 行构建 skill `ParsedToolCall`。
fn build_skill_call(skill_name: &str, args: &str) -> ParsedToolCall {
    let mut params = IndexMap::new();
    params.insert("skill".to_string(), Value::String(skill_name.to_string()));
    params.insert("args".to_string(), Value::String(args.to_string()));

    ParsedToolCall {
        tool_name: "skill".to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    }
}

#[cfg(test)]
#[path = "skill_action_tests.rs"]
mod tests;
