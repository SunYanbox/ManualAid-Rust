//! MCP server configuration as consumed by the client layer.
//! 客户端层消费的 MCP 服务器配置。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Transport used to reach an MCP server.
/// 访问 MCP 服务器所用的传输方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportKind {
    /// The client spawns the server as a child process and talks to it over
    /// stdin/stdout.
    /// 客户端将服务器作为子进程启动，经 stdin/stdout 通信。
    #[default]
    Stdio,
    /// The client reaches a remote server over Streamable HTTP.
    /// 客户端经 Streamable HTTP 访问远端服务器。
    Http,
}

impl McpTransportKind {
    /// The configuration label of this transport.
    /// 此传输的配置标签。
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
        }
    }

    /// Resolve a configuration label.
    ///
    /// Unknown labels yield `None` rather than a silent default, so the
    /// configuration layer can report the offending value and skip only that
    /// entry instead of failing the whole file.
    /// 解析配置标签。
    ///
    /// 未知标签返回 `None` 而非静默采用默认值，使配置层能报告问题值并只
    /// 跳过该条目，而不是让整个文件解析失败。
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "stdio" => Some(Self::Stdio),
            "http" => Some(Self::Http),
            _ => None,
        }
    }
}

/// Configuration of one MCP server as declared by the user.
/// 用户声明的单个 MCP 服务器配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name, unique among configured servers; also the middle segment
    /// of every exposed tool name it contributes.
    /// 服务器名，在已配置服务器中唯一；同时是其贡献的每个暴露工具名的中段。
    pub name: String,
    /// Transport to use.
    /// 使用的传输。
    #[serde(default)]
    pub transport: McpTransportKind,
    /// Executable to spawn; required by [`McpTransportKind::Stdio`].
    /// 要启动的可执行文件；[`McpTransportKind::Stdio`] 必需。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Arguments passed to `command`.
    /// 传给 `command` 的参数。
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra environment variables for the spawned process.
    /// 为所启动进程附加的环境变量。
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Remote endpoint URL; required by [`McpTransportKind::Http`].
    /// 远端端点 URL；[`McpTransportKind::Http`] 必需。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether this server is connected and its tools injected. Disabled
    /// servers are neither connected nor advertised.
    /// 是否连接此服务器并注入其工具。禁用的服务器既不连接也不暴露。
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl McpServerConfig {
    /// Check that the fields required by the selected transport are present.
    ///
    /// Returns a human-readable reason on failure; callers turn it into a
    /// configuration issue and skip this server without affecting the others.
    /// 检查所选传输必需的字段是否齐备。
    ///
    /// 失败时返回人类可读原因；调用方将其转为配置问题并跳过此服务器，
    /// 不影响其他服务器。
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("MCP server name must not be empty".to_string());
        }
        match self.transport {
            McpTransportKind::Stdio => {
                if self.command.as_deref().is_none_or(|c| c.trim().is_empty()) {
                    return Err(format!(
                        "MCP server `{}` uses the stdio transport but declares no `command`",
                        self.name
                    ));
                }
            }
            McpTransportKind::Http => {
                if self.url.as_deref().is_none_or(|u| u.trim().is_empty()) {
                    return Err(format!(
                        "MCP server `{}` uses the http transport but declares no `url`",
                        self.name
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Serde default for [`McpServerConfig::enabled`]: declared servers start
/// enabled, so a minimal entry works without spelling the flag out.
/// [`McpServerConfig::enabled`] 的 serde 默认值：声明即启用，使最小条目
/// 无需写出该标志。
fn default_true() -> bool {
    true
}
