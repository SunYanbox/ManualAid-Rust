//! Utilities for querying standard user directories.
//! 用于查询标准用户目录的工具模块。
//!
//! # Description
//! This module provides functions to retrieve platform-specific directories such as the user's home,
//! config, cache, and data directories. It wraps the `dirs` crate and falls back to environment
//! variables where appropriate. These paths are useful for storing application data in a
//! cross-platform manner.
//! # 描述
//! 此模块提供用于查询平台特定目录的函数，包括用户的主目录、配置目录、缓存目录和数据目录。
//! 它封装了 `dirs` crate，并在适当时回退到环境变量。这些路径有助于以跨平台方式存储应用程序数据。
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

/// Information about the current user's standard directories.
/// 当前用户标准目录的信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDirectories {
    /// Home directory (e.g. `/home/alice`, `C:\Users\Alice`).
    /// 主目录（例如 `/home/alice`、`C:\Users\Alice`）。
    pub home: PathBuf,
    /// Platform-specific config directory.
    /// 平台特定的配置目录。
    pub config: PathBuf,
    /// Platform-specific cache directory.
    /// 平台特定的缓存目录。
    pub cache: PathBuf,
    /// Platform-specific data directory.
    /// 平台特定的数据目录。
    pub data: PathBuf,
}

/// Returns the user's home directory.
/// 返回用户的主目录。
///
/// # Description
/// Uses the `dirs` crate which queries `$HOME` on Unix and
/// `USERPROFILE` on Windows.
/// # 描述
/// 使用 `dirs` crate，在 Unix 上查询 `$HOME`，在 Windows 上查询 `USERPROFILE`。
pub fn home_dir() -> CoreResult<PathBuf> {
    env_home().or_else(dirs::home_dir).ok_or_else(|| {
        CoreError::NotFound(
            "unable to determine home directory – neither $HOME nor USERPROFILE is set".into(),
        )
    })
}

/// The home directory from `USERPROFILE` on Windows and `HOME` elsewhere,
/// matching the documented contract. Falls back to OS-level APIs via
/// `dirs` when the variables are unset or empty.
fn env_home() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(home) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
            return Some(PathBuf::from(home));
        }
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Returns the user's config directory.
/// 返回用户的配置目录。
///
/// # Description
/// Platform paths:
/// - **Linux**: `$XDG_CONFIG_HOME` or `~/.config`
/// - **macOS**: `~/Library/Application Support`
/// - **Windows**: `{FOLDERID_RoamingAppData}` (typically `C:\Users\<user>\AppData\Roaming`)
///
/// **Note:** This does *not* return the configuration directory used by this agent.
/// The agent's configuration is stored at `~/.ManualAid/config.toml`, where `~` denotes the user's home directory.
/// This function is provided purely for the completeness of the crate's standard directory utilities.
/// # 描述
/// 平台路径：
/// - **Linux**：`$XDG_CONFIG_HOME` 或 `~/.config`
/// - **macOS**：`~/Library/Application Support`
/// - **Windows**：`{FOLDERID_RoamingAppData}`（通常为 `C:\Users\<user>\AppData\Roaming`）
///
/// **注意：** 此函数返回的*并非*本 Agent 工具使用的配置目录。
/// 本 Agent 的配置文件位于 `~/.ManualAid/config.toml`，其中 `~` 表示用户的主目录。
/// 提供此函数仅出于 crate 标准目录功能的完整性考虑。
pub fn config_dir() -> CoreResult<PathBuf> {
    dirs::config_dir().ok_or_else(|| {
        CoreError::NotFound(
            "unable to determine config directory – none of the platform-specific locations are available"
                .into(),
        )
    })
}

/// Returns the user's cache directory.
/// 返回用户的缓存目录。
///
/// # Description
/// Platform paths:
/// - **Linux**: `$XDG_CACHE_HOME` or `~/.cache`
/// - **macOS**: `~/Library/Caches`
/// - **Windows**: `{FOLDERID_LocalAppData}` (typically `C:\Users\<user>\AppData\Local`)
/// # 描述
/// 平台路径：
/// - **Linux**：`$XDG_CACHE_HOME` 或 `~/.cache`
/// - **macOS**：`~/Library/Caches`
/// - **Windows**：`{FOLDERID_LocalAppData}`（通常为 `C:\Users\<user>\AppData\Local`）
pub fn cache_dir() -> CoreResult<PathBuf> {
    dirs::cache_dir().ok_or_else(|| {
        CoreError::NotFound(
            "unable to determine cache directory – none of the platform-specific locations are available"
                .into(),
        )
    })
}

/// Returns the user's data directory.
/// 返回用户的数据目录。
///
/// # Description
/// Platform paths:
/// - **Linux**: `$XDG_DATA_HOME` or `~/.local/share`
/// - **macOS**: `~/Library/Application Support`
/// - **Windows**: `{FOLDERID_RoamingAppData}` (typically `C:\Users\<user>\AppData\Roaming`)
/// # 描述
/// 平台路径：
/// - **Linux**：`$XDG_DATA_HOME` 或 `~/.local/share`
/// - **macOS**：`~/Library/Application Support`
/// - **Windows**：`{FOLDERID_RoamingAppData}`（通常为 `C:\Users\<user>\AppData\Roaming`）
pub fn data_dir() -> CoreResult<PathBuf> {
    dirs::data_dir().ok_or_else(|| {
        CoreError::NotFound(
            "unable to determine data directory – none of the platform-specific locations are available"
                .into(),
        )
    })
}

/// Returns a [`UserDirectories`] struct with all standard paths.
/// 返回包含所有标准路径的 [`UserDirectories`] 结构体。
pub fn all_directories() -> CoreResult<UserDirectories> {
    Ok(UserDirectories {
        home: home_dir()?,
        config: config_dir()?,
        cache: cache_dir()?,
        data: data_dir()?,
    })
}
