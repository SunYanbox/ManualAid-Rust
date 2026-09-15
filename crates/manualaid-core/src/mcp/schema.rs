//! Conversion from an MCP tool's `inputSchema` JSON Schema document into the
//! flat parameter list the tool chain works with.
//! 从 MCP 工具的 `inputSchema` JSON Schema 文档转换为工具链使用的扁平参数
//! 列表。
//!
//! # Ordering
//! `serde_json` is built without `preserve_order`, so object keys — and with
//! them the parameters derived from `properties` — come out in sorted order
//! rather than the server's declaration order. Nothing downstream depends on
//! that order: parameters are looked up by name, and template rendering only
//! walks the list.
//! # 顺序
//! `serde_json` 未启用 `preserve_order`，因此对象键（以及由 `properties`
//! 导出的参数）按排序后的顺序输出，而非服务器的声明顺序。下游没有任何
//! 逻辑依赖该顺序：参数按名称查找，模板渲染只做遍历。

use serde_json::Value;

use super::tool::{McpParam, McpTool, exposed_name};

/// Type used for parameters whose schema declares no usable `type`.
/// schema 未声明可用 `type` 的参数所使用的类型。
const DEFAULT_KIND: &str = "string";

/// Build an [`McpTool`] from the raw fields a server reported.
/// 由服务器报告的原始字段构造 [`McpTool`]。
pub fn parse_tool(
    server_name: &str,
    tool_name: &str,
    description: &str,
    input_schema: Option<&Value>,
) -> McpTool {
    McpTool {
        server_name: server_name.to_string(),
        tool_name: tool_name.to_string(),
        exposed_name: exposed_name(server_name, tool_name),
        description: description.to_string(),
        params: parse_params(input_schema),
    }
}

/// Flatten a JSON Schema `object` document into a parameter list.
///
/// A schema that is not an object, or that carries no object `properties`
/// map, yields no parameters — the normal shape for a tool that takes none.
/// 将 JSON Schema `object` 文档扁平化为参数列表。
///
/// 非对象的 schema，或没有对象形式 `properties` 映射的 schema，不产生参数——
/// 这正是无参工具的常见形态。
pub fn parse_params(schema: Option<&Value>) -> Vec<McpParam> {
    let Some(Value::Object(root)) = schema else {
        return Vec::new();
    };
    let Some(Value::Object(properties)) = root.get("properties") else {
        return Vec::new();
    };
    let required: Vec<&str> = root
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    properties
        .iter()
        .map(|(name, property)| McpParam {
            name: name.clone(),
            kind: param_kind(property),
            required: required.contains(&name.as_str()),
            description: property
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
        .collect()
}

/// Map a property schema's `type` onto a parameter kind hint.
///
/// Missing, non-string and multi-type declarations all fall back to
/// [`DEFAULT_KIND`]. The hint only drives display and best-effort coercion;
/// it is never used for validation that could reject a well-formed call.
/// 将属性 schema 的 `type` 映射为参数类型提示。
///
/// 缺失、非字符串与多类型声明一律回退到 [`DEFAULT_KIND`]。该提示只影响
/// 展示与尽力而为的强制转换，绝不用于可能拒绝合法调用的校验。
fn param_kind(property: &Value) -> String {
    let kind = match property.get("type").and_then(Value::as_str) {
        Some("integer") => "integer",
        Some("number") => "number",
        Some("boolean") => "boolean",
        Some("array") => "array",
        Some("object") => "object",
        _ => DEFAULT_KIND,
    };
    kind.to_string()
}
