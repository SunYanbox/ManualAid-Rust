//! MCP (Model Context Protocol) client support.
//! MCP（模型上下文协议）客户端支持。
//!
//! # Description
//! Servers declared in the configuration are connected at startup and every
//! tool they expose is injected into the ordinary tool chain under the name
//! `mcp_<server>_<tool>`. The name is matched as a whole, so an underscore
//! inside the server or tool name stays unambiguous. These tools reach the
//! model through the same prompt list, the same wire-format parsers and the
//! same executor as the built-in ones; the only behavioural difference is
//! that every MCP call requires explicit user approval, because a server's
//! tool descriptions are untrusted input.
//! # 描述
//! 配置中声明的服务器在启动时连接，其暴露的每个工具都以
//! `mcp_<server>_<tool>` 的名称注入常规工具链。名称按整体匹配，因此服务器
//! 或工具名中的下划线不会产生歧义。这些工具与内置工具一样，经由同一份
//! 提示词列表、同样的线格式解析器与同一个执行器到达模型；唯一的行为差异是
//! 每次 MCP 调用都需要用户明确批准，因为服务器提供的工具描述属于不可信输入。

mod config;
mod connect;
mod schema;
mod store;
mod tool;

#[cfg(test)]
mod tests;

pub use config::{McpServerConfig, McpTransportKind};
pub use schema::{parse_params, parse_tool};
pub use store::{McpServerStatus, all_tools, enabled_tools, resolve_tool, server_status};
pub use tool::{McpParam, McpTool, exposed_name};

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::CoreResult;
use crate::text::t_fmt;
use crate::tools::ToolResult;
use store::ServerState;

/// Connect every configured server and install the tools they expose.
/// 连接每个已配置服务器并安装它们暴露的工具。
///
/// # Description
/// Servers that fail validation, are disabled, or fail to connect are kept in
/// the store with the reason recorded, so the management menu can show why a
/// server contributed nothing; they never affect the other servers or the
/// built-in tools. The call itself only fails when the store cannot be
/// updated.
/// # 描述
/// 校验失败、被禁用或连接失败的服务器仍会保留在存储中并记录原因，以便管理
/// 菜单展示某服务器为何未贡献任何工具；它们绝不影响其他服务器或内置工具。
/// 仅当存储无法更新时，本调用才会失败。
pub async fn connect_all(servers: &[McpServerConfig]) -> CoreResult<()> {
    let mut states = Vec::with_capacity(servers.len());
    for server in servers {
        if let Err(reason) = server.validate() {
            states.push(ServerState {
                config: server.clone(),
                tools: Vec::new(),
                error: Some(reason),
                client: None,
            });
            continue;
        }
        if !server.enabled {
            states.push(ServerState {
                config: server.clone(),
                tools: Vec::new(),
                error: None,
                client: None,
            });
            continue;
        }
        match connect::open(server).await {
            Ok((client, tools)) => states.push(ServerState {
                config: server.clone(),
                tools,
                error: None,
                client: Some(client),
            }),
            Err(reason) => states.push(ServerState {
                config: server.clone(),
                tools: Vec::new(),
                error: Some(reason),
                client: None,
            }),
        }
    }
    // Installing replaces the previous list, so any connection that is no
    // longer wanted is dropped here and closed by the service loop.
    // 安装会替换先前的列表，因此不再需要的连接在此被丢弃并由服务循环关闭。
    store::install(states);
    Ok(())
}

/// Drop every server connection and clear the tools they contributed.
/// 断开所有服务器连接并清空它们贡献的工具。
///
/// # Description
/// Called before the process exits so child servers are not left behind on
/// platforms where dropping the handles would be skipped.
/// # 描述
/// 在进程退出前调用，以免在丢弃句柄会被跳过的平台上留下子服务器进程。
pub async fn shutdown() -> CoreResult<()> {
    // Take the clients out first so the store no longer hands out peers for a
    // connection that is being closed.
    // 先取出客户端，使存储不再为一个正在关闭的连接发放句柄。
    let clients = store::take_clients();
    for client in clients {
        client.close().await;
    }
    store::reset();
    Ok(())
}

/// Invoke an MCP tool by its exposed name.
/// 按暴露名调用 MCP 工具。
///
/// # Description
/// The result is reported as a [`ToolResult`] rather than a `CoreError`, so
/// a failing server degrades into an ordinary failed tool call that the model
/// can react to, instead of aborting the round.
/// # 描述
/// 结果以 [`ToolResult`] 而非 `CoreError` 报告，使服务器故障降级为一次普通
/// 的失败工具调用，供模型自行应对，而不是中止整轮对话。
pub async fn call_tool(exposed_name: &str, params: &IndexMap<String, Value>) -> ToolResult {
    let Some(tool) = resolve_tool(exposed_name) else {
        return ToolResult::failure(
            exposed_name,
            t_fmt("mcp.error.unknown_tool", &[("name", exposed_name)]),
        );
    };
    let Some(peer) = store::peer_for(&tool.server_name) else {
        return ToolResult::failure(
            exposed_name,
            t_fmt(
                "mcp.error.not_connected",
                &[("server", tool.server_name.as_str())],
            ),
        );
    };
    connect::call(&peer, &tool, params).await
}

/// Clear the store. Hidden from docs because it exists for tests.
/// 清空存储。文档中隐藏，因为它供测试使用。
#[doc(hidden)]
pub fn reset() {
    store::reset();
}
