use serde::{Deserialize, Serialize};
use std::fmt;

/// # Description
/// Unified error type for the entire library.
/// Most variants carry simple `String` payloads, except for `Execution` which
/// carries a structured payload. This design ensures the type remains fully
/// serializable (many underlying error types from std and ecosystem crates
/// do not implement `Serialize` / `Deserialize`).
/// # 描述
/// 整个库的统一错误类型。
/// 除 `Execution` 变体携带结构化负载外，大多数变体都携带简单的 `String` 负载。
/// 这种设计确保了该类型完全可序列化
/// （许多来自 std 和生态 crate 的底层错误类型未实现 `Serialize` / `Deserialize`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail")]
pub enum CoreError {
    /// Filesystem or I/O operation failed.
    /// 文件系统或 I/O 操作失败。
    Io(String),
    /// Configuration parsing or validation failure.
    /// 配置解析或校验失败。
    Config(String),
    /// A required resource was not found.
    /// 未找到所需资源。
    NotFound(String),
    /// The operation was denied due to insufficient permissions.
    /// 由于权限不足，操作被拒绝。
    PermissionDenied(String),
    /// A string or data stream could not be parsed.
    /// 字符串或数据流无法解析。
    Parse(String),
    /// The supplied path is invalid or malformed.
    /// 提供的路径无效或格式错误。
    InvalidPath(String),
    /// Text filtering encountered an issue.
    /// 文本过滤遇到问题。
    Filter(String),
    /// An external command finished with a non-zero exit code.
    /// 外部命令以非零退出码结束。
    Execution {
        /// The command that was executed.
        /// 执行的命令。
        command: String,
        /// The exit code returned by the command.
        /// 命令返回的退出码。
        exit_code: i32,
        /// Standard error output from the command.
        /// 命令的标准错误输出。
        stderr: String,
    },
    /// Catch-all for errors that do not fit above.
    /// 用于不适合上述分类的错误兜底。
    Other(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::Config(msg) => write!(f, "config error: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "permission denied: {msg}"),
            Self::Parse(msg) => write!(f, "parse error: {msg}"),
            Self::InvalidPath(msg) => write!(f, "invalid path: {msg}"),
            Self::Filter(msg) => write!(f, "filter error: {msg}"),
            Self::Execution {
                command,
                exit_code,
                stderr,
            } => {
                write!(
                    f,
                    "command `{command}` exited with code {exit_code}: {stderr}"
                )
            }
            Self::Other(msg) => write!(f, "Other error: {msg}"),
        }
    }
}

impl std::error::Error for CoreError {}

// 便捷转换实现：将标准库和生态 crate 的错误类型自动转换为 CoreError

// 将 std::io::Error 转换为 CoreError，根据错误类型细分为 NotFound、PermissionDenied 或通用 Io
impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound(err.to_string()),
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied(err.to_string()),
            _ => Self::Io(err.to_string()),
        }
    }
}

// 将 TOML 反序列化错误转换为配置类错误
impl From<toml::de::Error> for CoreError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config(format!("invalid TOML: {err}"))
    }
}

// 将 TOML 序列化错误转换为配置类错误
impl From<toml::ser::Error> for CoreError {
    fn from(err: toml::ser::Error) -> Self {
        Self::Config(format!("failed to serialize TOML: {err}"))
    }
}

/// # Description
/// Alias for `Result<T, CoreError>` used throughout the library.
/// # 描述
/// 整个库使用的 `Result<T, CoreError>` 类型别名。
pub type CoreResult<T> = Result<T, CoreError>;
