//! Process environment helpers used by the command handlers: the default
//! startup message, the current working directory and the user home
//! directory, returning localized error messages on failure.
//! 命令处理使用的进程环境辅助：默认启动消息、当前工作目录与用户主目录，
//! 失败时均返回本地化错误信息。

use std::path::PathBuf;

use manualaid_core::user_dir;

use crate::t_fmt;

/// The default startup message, with the crate version interpolated from the
/// crate manifest.
/// 默认启动消息，从 crate 清单中插入 crate 版本。
pub(crate) fn default_message() -> String {
    crate::t_fmt("manual-aid-running", &[("version", env!("CARGO_PKG_VERSION"))])
}

/// The current working directory, or a localized error message.
/// 当前工作目录，失败时返回本地化错误信息。
pub(crate) fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))
}

/// The user home directory, or a localized error message.
/// 用户主目录，失败时返回本地化错误信息。
pub(crate) fn home_dir() -> Result<PathBuf, String> {
    user_dir::home_dir().map_err(|e| t_fmt("cli.error.home", &[("error", &e.to_string())]))
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
