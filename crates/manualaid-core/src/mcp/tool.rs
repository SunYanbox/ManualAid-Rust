//! MCP tool metadata: the runtime counterpart of the compile-time
//! [`ToolKind`](crate::tools::ToolKind) enum.
//! MCP 工具元数据：编译期 [`ToolKind`](crate::tools::ToolKind) 枚举的运行时
//! 对应物。

use serde::{Deserialize, Serialize};

/// Build the name an MCP tool is exposed under.
///
/// The result is matched as a whole (see [`crate::mcp::resolve_tool`]) rather
/// than split back into its server and tool parts, so an underscore inside
/// either part stays unambiguous.
/// 构造 MCP 工具的暴露名。
///
/// 结果按整体匹配（见 [`crate::mcp::resolve_tool`]），不会被反解为服务器与
/// 工具两部分，因此任一部分含下划线也不会产生歧义。
pub fn exposed_name(server: &str, tool: &str) -> String {
    format!("mcp_{server}_{tool}")
}

/// One parameter of an MCP tool, derived from the server's JSON Schema.
/// MCP 工具的一个参数，由服务器的 JSON Schema 导出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpParam {
    /// Parameter name as it appears in the tool's `inputSchema`.
    /// 参数在工具 `inputSchema` 中的名称。
    pub name: String,
    /// Type hint (`"string"`, `"integer"`, ...).
    /// 类型提示（`"string"`、`"integer"` 等）。
    pub kind: String,
    /// Whether the server lists this parameter as required.
    /// 服务器是否将此参数列为必需。
    pub required: bool,
    /// Parameter description from the schema; empty when the server omits it.
    /// schema 中的参数描述；服务器未提供时为空。
    pub description: String,
}

/// One tool discovered on a connected MCP server.
/// 在已连接 MCP 服务器上发现的一个工具。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTool {
    /// Name of the server this tool was discovered on.
    /// 发现此工具的服务器名称。
    pub server_name: String,
    /// Tool name exactly as the server reported it.
    /// 服务器报告的原始工具名称。
    pub tool_name: String,
    /// Name this tool is exposed under in the tool chain.
    /// 该工具在工具链中的暴露名。
    pub exposed_name: String,
    /// Tool description as reported by the server. Untrusted input: it is
    /// shown to the model but never treated as instructions.
    /// 服务器报告的工具描述。属不可信输入：会展示给模型，但绝不当作指令。
    pub description: String,
    /// Parameters derived from the tool's `inputSchema`.
    /// 由工具 `inputSchema` 导出的参数。
    pub params: Vec<McpParam>,
}
