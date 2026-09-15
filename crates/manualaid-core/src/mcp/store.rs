//! Process-wide MCP runtime state: which servers are configured, which
//! tools they contributed, and why a server contributed none.
//! 进程级 MCP 运行时状态：哪些服务器已配置、它们贡献了哪些工具，以及某
//! 服务器为何没有贡献任何工具。
//!
//! # Description
//! [`install`] replaces the whole server list in one write, so readers never
//! observe a mix of old and new state. Exposed names are de-duplicated across
//! servers on install: the first server contributing a name wins, because
//! `mcp_a_b_c` is ambiguous between server `a` with tool `b_c` and server
//! `a_b` with tool `c`.
//! # 描述
//! [`install`] 一次性替换整个服务器列表，读取方不会观察到新旧状态的混合。
//! 暴露名在安装时跨服务器去重：先贡献该名称的服务器获胜，因为 `mcp_a_b_c`
//! 在服务器 `a` 的工具 `b_c` 与服务器 `a_b` 的工具 `c` 之间存在歧义。

use std::collections::HashSet;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::file_io;
use crate::mcp::config::{McpServerConfig, McpTransportKind};
use crate::mcp::tool::McpTool;

/// One configured server together with the tools discovered on it.
/// 一个已配置的服务器及其上发现的工具。
pub(crate) struct ServerState {
    /// The configuration this state was built from.
    /// 构建此状态所用的配置。
    pub config: McpServerConfig,
    /// Tools discovered on the server; empty when the server is disabled or
    /// the connection failed.
    /// 服务器上发现的工具；服务器被禁用或连接失败时为空。
    pub tools: Vec<McpTool>,
    /// Why the server contributed no tools, when that is the case.
    /// 服务器未贡献任何工具的原因（若确实如此）。
    pub error: Option<String>,
}

/// A read-only snapshot of one configured server.
/// 一个已配置服务器的只读快照。
///
/// # Description
/// Carries the connection outcome to management surfaces without handing out
/// the live state, so a menu can explain why a server contributed no tools.
/// # 描述
/// 将连接结果带往管理界面而不交出实时状态，使菜单能解释某服务器为何未贡献
/// 任何工具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerStatus {
    /// Server name from the configuration.
    /// 配置中的服务器名。
    pub name: String,
    /// Transport the server is configured to use.
    /// 该服务器配置使用的传输。
    pub transport: McpTransportKind,
    /// Whether the server is enabled.
    /// 服务器是否启用。
    pub enabled: bool,
    /// Number of tools discovered on the server.
    /// 在该服务器上发现的工具数量。
    pub tool_count: usize,
    /// Why the server contributed no tools, when that is the case.
    /// 该服务器未贡献任何工具的原因（若确实如此）。
    pub error: Option<String>,
}

/// The in-memory MCP store, guarded by a single `RwLock` so readers never
/// observe a mix of old and new state.
/// 内存中的 MCP 存储；由单一 `RwLock` 保护，读取方不会观察到新旧状态的混合。
struct McpStore {
    servers: Vec<ServerState>,
}

static STORE: RwLock<McpStore> = RwLock::new(McpStore {
    servers: Vec::new(),
});

/// Acquire the store read guard, recovering from a poisoned lock with a
/// one-time warning. Recovery is safe because the next `connect_all` rebuilds
/// the state from configuration.
/// 获取存储读锁；遇到被污染的锁时记录一次性警告并恢复。恢复是安全的，因为
/// 下一次 `connect_all` 会依据配置重建状态。
fn read_store() -> RwLockReadGuard<'static, McpStore> {
    STORE.read().unwrap_or_else(|poisoned| {
        file_io::warn_poisoned_lock("MCP_STORE");
        poisoned.into_inner()
    })
}

/// Acquire the store write guard under the same recovery policy as
/// [`read_store`].
/// 在与 [`read_store`] 相同的恢复策略下获取存储写锁。
fn write_store() -> RwLockWriteGuard<'static, McpStore> {
    STORE.write().unwrap_or_else(|poisoned| {
        file_io::warn_poisoned_lock("MCP_STORE");
        poisoned.into_inner()
    })
}

/// Replace the whole server list, dropping tools whose exposed name was
/// already claimed by an earlier server.
/// 替换整个服务器列表，丢弃暴露名已被更靠前服务器占用的工具。
pub(crate) fn install(servers: Vec<ServerState>) {
    let mut seen: HashSet<String> = HashSet::new();
    let servers = servers
        .into_iter()
        .map(|mut server| {
            server
                .tools
                .retain(|tool| seen.insert(tool.exposed_name.clone()));
            server
        })
        .collect();
    write_store().servers = servers;
}

/// Every tool contributed by any configured server, enabled or not.
/// 所有已配置服务器贡献的工具，无论是否启用。
pub fn all_tools() -> Vec<McpTool> {
    read_store()
        .servers
        .iter()
        .flat_map(|server| server.tools.iter().cloned())
        .collect()
}

/// Every tool contributed by an enabled server. Disabled servers contribute
/// nothing, so their tools are never advertised or callable.
/// 已启用服务器贡献的每个工具。禁用的服务器不贡献任何内容，其工具既不会
/// 被展示也不可调用。
pub fn enabled_tools() -> Vec<McpTool> {
    read_store()
        .servers
        .iter()
        .filter(|server| server.config.enabled)
        .flat_map(|server| server.tools.iter().cloned())
        .collect()
}

/// Resolve an exposed tool name against the enabled servers.
/// 针对已启用服务器解析暴露的工具名。
pub fn resolve_tool(exposed_name: &str) -> Option<McpTool> {
    read_store()
        .servers
        .iter()
        .filter(|server| server.config.enabled)
        .flat_map(|server| server.tools.iter())
        .find(|tool| tool.exposed_name == exposed_name)
        .cloned()
}

/// Snapshot every configured server in declaration order.
/// 按声明顺序快照每个已配置服务器。
pub fn server_status() -> Vec<McpServerStatus> {
    read_store()
        .servers
        .iter()
        .map(|server| McpServerStatus {
            name: server.config.name.clone(),
            transport: server.config.transport,
            enabled: server.config.enabled,
            tool_count: server.tools.len(),
            error: server.error.clone(),
        })
        .collect()
}

/// Clear the store and drop the remembered server list.
/// 清空存储并丢弃已记住的服务器列表。
///
/// Hidden from docs because it exists for tests to restore state.
/// 文档中隐藏，因为它供测试恢复状态用。
#[doc(hidden)]
pub fn reset() {
    write_store().servers.clear();
}
