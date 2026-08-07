//! The unified tool layer: definition (parameters, descriptions, semantics)
//! and execution (async `run`) live in the [`ToolKind`] enum, so the
//! executor, parser and prompt builder share one source of truth.
//! 统一工具层：定义（参数、描述、语义）与执行（异步 `run`）都在
//! [`ToolKind`] 枚举中，执行器、解析器与提示词构建器共享同一事实来源。

pub mod edit;
pub mod read;
pub mod shell;
pub mod skill;
pub mod tool;
pub mod write;

pub use tool::{ParamSemantic, ToolCallFormat, ToolKind, ToolParam, ToolResult, params_summary_of};

/// The static list of every built-in tool, in a stable order used by the
/// prompt builder and the executor's routing.
/// 每个内置工具的静态列表（顺序稳定），供提示词构建器与执行器路由使用。
pub fn all_tools() -> &'static [ToolKind] {
    &[
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Write,
        ToolKind::Shell,
        ToolKind::Skill,
    ]
}

/// Extract a `String` value from a parameter map.
/// 从参数映射中提取 `String` 值。
pub(crate) fn get_string(
    params: &indexmap::IndexMap<String, serde_json::Value>,
    key: &str,
) -> Option<String> {
    params.get(key).and_then(|value| match value {
        serde_json::Value::String(s) => Some(s.clone()),
        _ => value.as_str().map(|s| s.to_string()),
    })
}

/// Extract an `i64` value from a parameter map.
/// 从参数映射中提取 `i64` 值。
pub(crate) fn get_i64(
    params: &indexmap::IndexMap<String, serde_json::Value>,
    key: &str,
) -> Option<i64> {
    params.get(key).and_then(|value| value.as_i64())
}

/// Extract a `bool` value from a parameter map.
/// 从参数映射中提取 `bool` 值。
pub(crate) fn get_bool(
    params: &indexmap::IndexMap<String, serde_json::Value>,
    key: &str,
) -> Option<bool> {
    params.get(key).and_then(|value| value.as_bool())
}
