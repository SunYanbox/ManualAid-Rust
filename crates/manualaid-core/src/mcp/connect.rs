//! Connection layer for MCP servers: spawn a stdio server, complete the MCP
//! handshake, discover the tools it exposes, and invoke those tools.
//! MCP 服务器的连接层：启动 stdio 服务器、完成 MCP 握手、发现其暴露的工具，
//! 并调用这些工具。
//!
//! # Timeouts
//! Spawning, the handshake and tool discovery share one ten-second budget;
//! a single `tools/call` gets thirty seconds. Both are implementation
//! constants rather than configuration, so a hung server degrades into a
//! recorded error instead of blocking the session.
//! # 超时
//! 启动、握手与工具发现共用一个十秒预算；单次 `tools/call` 为三十秒。两者
//! 都是实现内常量而非配置项，因此挂起的服务器会降级为一条记录的错误，
//! 而不是阻塞会话。
//!
//! # Protocol coverage
//! Calls go through [`Peer::call_tool`], which returns a final result
//! directly. SEP-2322 `input_required` rounds are therefore not driven; a
//! server that answers a call with an intermediate result surfaces as a
//! failed call rather than hanging.
//! # 协议覆盖范围
//! 调用经由 [`Peer::call_tool`] 完成，它直接返回最终结果。因此不会驱动
//! SEP-2322 的 `input_required` 轮次；若服务器以中间结果应答，该调用会
//! 呈现为失败而不是挂起。
//!
//! # Windows command resolution
//! The child process is spawned through `tokio::process::Command`, which does
//! not consult `PATHEXT`. On Windows a configuration that names a shim such as
//! `npx` must spell out the resolved file (`npx.cmd`) instead.
//! # Windows 命令解析
//! 子进程经 `tokio::process::Command` 启动，它不查询 `PATHEXT`。在 Windows
//! 上，配置若指向 `npx` 这类垫片脚本，必须写出解析后的文件（`npx.cmd`）。

use std::process::Stdio;
use std::time::Duration;

use indexmap::IndexMap;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ContentBlock, JsonObject, ResourceContents,
};
use rmcp::service::RunningService;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{Peer, RoleClient, serve_client};
use serde_json::Value;

use crate::mcp::config::{McpServerConfig, McpTransportKind};
use crate::mcp::schema::parse_tool;
use crate::mcp::tool::McpTool;
use crate::text::t_fmt;
use crate::tools::ToolResult;

/// Budget for spawning a server, completing the handshake and listing its
/// tools. A server that exceeds it is recorded as failed and skipped.
/// 启动服务器、完成握手与列出其工具的预算。超出的服务器会被记为失败并跳过。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Budget for one `tools/call`. A server that exceeds it yields a failed tool
/// call rather than blocking the round.
/// 单次 `tools/call` 的预算。超出的服务器产生一次失败的工具调用，而不是
/// 阻塞整轮对话。
const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Budget for draining a server during shutdown; the process is killed once it
/// elapses.
/// 关闭期间等待服务器退出的预算；超出后进程会被杀掉。
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

/// A live connection to one MCP server.
/// 到一个 MCP 服务器的实时连接。
pub(crate) struct McpClient {
    /// Handle used to issue requests. Cloned out of the store before awaiting
    /// so no lock is held across a request.
    /// 用于发起请求的句柄。在 await 之前从存储中克隆出来，因此请求期间不持有
    /// 任何锁。
    peer: Peer<RoleClient>,
    /// The running service loop. Kept solely so the connection stays open;
    /// dropping it cancels the connection.
    /// 运行中的服务循环。保留它只为让连接保持打开；丢弃它会取消连接。
    service: RunningService<RoleClient, ()>,
}

impl McpClient {
    /// Clone the request handle, leaving the client itself in the store.
    /// 克隆请求句柄，客户端本身留在存储中。
    pub(crate) fn peer(&self) -> Peer<RoleClient> {
        self.peer.clone()
    }

    /// Close the connection, giving the server a bounded window to exit.
    /// 关闭连接，给服务器一段有限的退出时间。
    pub(crate) async fn close(mut self) {
        let _ = self.service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
    }
}

/// Connect to a server and discover the tools it exposes.
/// 连接服务器并发现其暴露的工具。
///
/// # Description
/// The failure is a human-readable reason rather than a `CoreError`, because
/// a server that cannot be reached must not fail the caller: it is recorded
/// against that server and skipped.
/// # 描述
/// 失败时返回人类可读原因而非 `CoreError`，因为无法访问的服务器不应让调用方
/// 失败：它会被记录在该服务器名下并跳过。
pub(crate) async fn open(config: &McpServerConfig) -> Result<(McpClient, Vec<McpTool>), String> {
    match config.transport {
        McpTransportKind::Stdio => open_stdio(config).await,
        McpTransportKind::Http => open_http(config).await,
    }
}

/// Spawn a stdio server, hand it to the MCP client loop and list its tools.
/// 启动 stdio 服务器，交给 MCP 客户端循环并列出其工具。
async fn open_stdio(config: &McpServerConfig) -> Result<(McpClient, Vec<McpTool>), String> {
    let command = config.command.as_deref().unwrap_or_default();
    let mut child = tokio::process::Command::new(command);
    child.args(&config.args);
    for (key, value) in &config.env {
        child.env(key, value);
    }

    // stderr is discarded rather than inherited: the server's own logging must
    // not be interleaved with the interactive session.
    // stderr 被丢弃而非继承：服务器自身的日志不得混入交互会话。
    let transport = TokioChildProcess::builder(child)
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| connect_failed(config, &error.to_string()))?
        .0;

    finish_handshake(config, transport).await
}

/// Reach a remote server over Streamable HTTP and list its tools.
/// 经 Streamable HTTP 访问远端服务器并列出其工具。
///
/// # Description
/// The endpoint is used exactly as configured: redirects are disabled by the
/// transport, so a URL the user did not write can never be reached.
/// # 描述
/// 端点按配置原样使用：传输层禁用了重定向，因此用户未曾写下的 URL 绝无可能
/// 被访问。
async fn open_http(config: &McpServerConfig) -> Result<(McpClient, Vec<McpTool>), String> {
    let url = config.url.as_deref().unwrap_or_default();
    let transport = StreamableHttpClientTransport::from_uri(url.to_string());
    finish_handshake(config, transport).await
}

/// Complete the MCP handshake on a freshly built transport and list its tools.
/// 在新建的传输上完成 MCP 握手并列出其工具。
///
/// # Description
/// Both transports differ only in how the connection is established; the
/// handshake, the discovery call and the conversion of what a server reports
/// are shared so the two paths cannot drift apart.
/// # 描述
/// 两种传输只在如何建立连接上不同；握手、发现调用以及服务器报告内容的转换
/// 都是共用的，使两条路径不会各自漂移。
async fn finish_handshake<T, E, A>(
    config: &McpServerConfig,
    transport: T,
) -> Result<(McpClient, Vec<McpTool>), String>
where
    T: rmcp::transport::IntoTransport<RoleClient, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let service = tokio::time::timeout(CONNECT_TIMEOUT, serve_client((), transport))
        .await
        .map_err(|_| connect_timeout(config))?
        .map_err(|error| connect_failed(config, &error.to_string()))?;

    let peer = service.peer().clone();
    let discovered = tokio::time::timeout(CONNECT_TIMEOUT, peer.list_all_tools())
        .await
        .map_err(|_| connect_timeout(config))?
        .map_err(|error| connect_failed(config, &error.to_string()))?;

    let tools = discovered
        .iter()
        .map(|tool| {
            let schema = tool.schema_as_json_value();
            parse_tool(
                &config.name,
                tool.name.as_ref(),
                tool.description.as_deref().unwrap_or_default(),
                Some(&schema),
            )
        })
        .collect();

    Ok((McpClient { peer, service }, tools))
}

/// Invoke one tool on a connected server.
/// 在已连接的服务器上调用一个工具。
///
/// # Description
/// Every failure — timeout, transport error, or a result the server marked as
/// an error — becomes a [`ToolResult`], so the model can react to it instead
/// of the round aborting.
/// # 描述
/// 每种失败——超时、传输错误或服务器标记为错误的结果——都会变成一个
/// [`ToolResult`]，供模型自行应对，而不是中止整轮对话。
pub(crate) async fn call(
    peer: &Peer<RoleClient>,
    tool: &McpTool,
    params: &IndexMap<String, Value>,
) -> ToolResult {
    let mut arguments = JsonObject::new();
    for (key, value) in params {
        arguments.insert(key.clone(), value.clone());
    }
    let mut request = CallToolRequestParams::new(tool.tool_name.clone());
    request.arguments = Some(arguments);

    match tokio::time::timeout(CALL_TIMEOUT, peer.call_tool(request)).await {
        Err(_) => {
            let seconds = CALL_TIMEOUT.as_secs().to_string();
            ToolResult::failure(
                &tool.exposed_name,
                t_fmt(
                    "mcp.error.call_timeout",
                    &[
                        ("tool", tool.exposed_name.as_str()),
                        ("seconds", seconds.as_str()),
                    ],
                ),
            )
        }
        Ok(Err(error)) => ToolResult::failure(
            &tool.exposed_name,
            t_fmt(
                "mcp.error.call_failed",
                &[
                    ("tool", tool.exposed_name.as_str()),
                    ("reason", &error.to_string()),
                ],
            ),
        ),
        Ok(Ok(result)) => to_tool_result(&tool.exposed_name, result),
    }
}

/// Map a server result onto the tool chain's result type.
/// 将服务器结果映射为工具链的结果类型。
///
/// # Description
/// Exposed to the rest of the crate so the mapping can be unit tested without
/// a live server: it is the only part of this module that is pure.
/// # 描述
/// 对 crate 内其余部分公开，使该映射无需真实服务器即可单元测试：它是本模块
/// 中唯一纯粹的部分。
pub(crate) fn to_tool_result(exposed_name: &str, result: CallToolResult) -> ToolResult {
    let output = render_content(&result.content);
    if result.is_error == Some(true) {
        ToolResult::failure(exposed_name, output)
    } else {
        ToolResult::success(exposed_name, output, false)
    }
}

/// Join every content block into the single text body the chain carries.
/// 将所有内容块拼接为工具链承载的单一文本体。
fn render_content(blocks: &[ContentBlock]) -> String {
    let parts: Vec<String> = blocks.iter().map(render_block).collect();
    parts.join("\n")
}

/// Render one content block, standing in for the kinds the chain cannot carry.
/// 渲染单个内容块，为工具链无法承载的种类提供占位。
///
/// # Description
/// Only text is passed through verbatim. Binary and linked payloads become
/// bracketed markers naming what the server returned, which keeps the output
/// honest about what was dropped instead of inventing a representation.
/// # 描述
/// 只有文本原样传递。二进制与链接负载变成带方括号的标记，说明服务器返回了
/// 什么；这比编造一种表示更诚实，也让人知道丢弃了什么。
fn render_block(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Text(text) => text.text.clone(),
        ContentBlock::Image(image) => format!("[image: {}]", image.mime_type),
        ContentBlock::Audio(audio) => format!("[audio: {}]", audio.mime_type),
        ContentBlock::Resource(embedded) => match &embedded.resource {
            ResourceContents::TextResourceContents { text, .. } => text.clone(),
            _ => "[embedded resource]".to_string(),
        },
        ContentBlock::ResourceLink(link) => format!("[resource: {}]", link.uri),
        _ => "[unsupported content]".to_string(),
    }
}

/// Reason string for a server that could not be reached.
/// 服务器无法访问时的原因字符串。
fn connect_failed(config: &McpServerConfig, reason: &str) -> String {
    t_fmt(
        "mcp.error.connect_failed",
        &[("server", config.name.as_str()), ("reason", reason)],
    )
}

/// Reason string for a server that exceeded [`CONNECT_TIMEOUT`].
/// 服务器超出 [`CONNECT_TIMEOUT`] 时的原因字符串。
fn connect_timeout(config: &McpServerConfig) -> String {
    let seconds = CONNECT_TIMEOUT.as_secs().to_string();
    t_fmt(
        "mcp.error.connect_timeout",
        &[
            ("server", config.name.as_str()),
            ("seconds", seconds.as_str()),
        ],
    )
}
