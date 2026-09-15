//! Unit tests for the MCP runtime layer.
//! MCP 运行时层的单元测试。

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use super::store::ServerState;
use super::*;

mod config;
mod schema;
mod store;

/// Serializes the tests that touch the process-wide store, which they all
/// share; without it, one test resetting the store could run in the middle of
/// another test's assertions.
/// 串行化会触碰进程级存储的测试（它们共享同一份存储）；否则某个重置存储的
/// 测试可能插进另一个测试的断言之间。
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` with the store lock held. A poisoned lock is recovered so one
/// failing test does not cascade into the rest.
/// 持有存储锁运行 `f`。锁被污染时恢复，使单个测试失败不会连锁影响其余测试。
fn with_store_lock<T>(f: impl FnOnce() -> T) -> T {
    let _guard = STORE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f()
}

/// Drive `future` to completion on a fresh current-thread runtime, so a test
/// can hold the store lock across the whole await.
/// 在新建立的单线程运行时上把 `future` 驱动到完成，使测试能在整个 await
/// 期间持有存储锁。
fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build a test runtime")
        .block_on(future)
}

/// A minimal valid stdio server config.
/// 一个最小的合法 stdio 服务器配置。
fn stdio(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        transport: McpTransportKind::Stdio,
        command: Some("dummy-server".to_string()),
        args: Vec::new(),
        env: HashMap::new(),
        url: None,
        enabled: true,
    }
}

/// A minimal valid http server config.
/// 一个最小的合法 http 服务器配置。
fn http(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        transport: McpTransportKind::Http,
        command: None,
        args: Vec::new(),
        env: HashMap::new(),
        url: Some("https://example.com/mcp".to_string()),
        enabled: true,
    }
}

/// A tool reported by `server` under the raw name `name`.
/// 服务器 `server` 以原始名称 `name` 报告的工具。
fn tool(server: &str, name: &str) -> McpTool {
    parse_tool(server, name, "description", None)
}

/// One server state carrying `tools` and no recorded error.
/// 一个携带 `tools` 且未记录错误的服务器状态。
fn state(config: McpServerConfig, tools: Vec<McpTool>) -> ServerState {
    ServerState {
        config,
        tools,
        error: None,
    }
}

/// Install exactly one server.
/// 仅安装一个服务器。
fn install_one(config: McpServerConfig, tools: Vec<McpTool>) {
    install_many(vec![state(config, tools)]);
}

/// Install several servers in one write.
/// 一次性安装多个服务器。
fn install_many(states: Vec<ServerState>) {
    // Qualified through `super` on purpose: the test submodule below is also
    // named `store`, and a bare path would resolve to that instead of the
    // store under test.
    // 特意经由 `super` 限定：下面的测试子模块同样名为 `store`，裸路径会解析
    // 到它而不是被测的存储模块。
    super::store::install(states);
}
