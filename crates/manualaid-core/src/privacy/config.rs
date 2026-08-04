//! # Description
//! Loading and merging of the `[privacy_mask_extension]` tables from the
//! global (`~/.ManualAid/config.toml`) and project
//! (`<workspace>/.ManualAid/config.toml`) configuration files.
//!
//! ```toml
//! [privacy_mask_extension.regex]
//! ExamApiKey = "^sk-[A-Za-z0-9]{7}$"
//!
//! [privacy_mask_extension.literal]
//! UserName = "Alice"
//! ```
//! # 描述
//! 从全局（`~/.ManualAid/config.toml`）与项目
//! （`<workspace>/.ManualAid/config.toml`）配置文件加载并合并
//! `[privacy_mask_extension]` 表。

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::{CoreError, CoreResult};
use crate::user_dir;

/// User-configured privacy mask extensions.
///
/// `regex` holds `类型 = 正则字符串` pairs compiled as regular expressions;
/// `literal` holds exact-match pairs whose values are matched as literal
/// substrings (non-regex). Project config overrides the global config per
/// key, in memory only — config files are never written.
/// 用户配置的隐私掩码扩展。
///
/// `regex` 存放 `类型 = 正则字符串` 键值对，按正则编译；`literal` 存放
/// 精确匹配键值对，值按普通文本（非正则）在任意位置做子串匹配。项目
/// 配置按 key 逐项覆盖全局配置，仅内存生效——绝不写回配置文件。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivacyMaskExtension {
    /// `类型 = 正则字符串` pairs.
    /// `类型 = 正则字符串` 键值对。
    pub regex: HashMap<String, String>,
    /// Exact-match pairs: values matched as literal substrings anywhere.
    /// 精确匹配键值对：值作为字面量在任意位置子串匹配。
    pub literal: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ConfigFile {
    privacy_mask_extension: ExtensionTable,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ExtensionTable {
    regex: HashMap<String, String>,
    literal: HashMap<String, String>,
}

impl PrivacyMaskExtension {
    /// Load and merge the global and project `[privacy_mask_extension]`
    /// tables. Project keys override global keys (in memory only; config
    /// files are never written). Overridden global keys are logged at info
    /// level with their full path, e.g.
    /// `privacy_mask_extension.regex.ExamApiKey`.
    /// 加载并合并全局与项目 `[privacy_mask_extension]` 表。项目键覆盖
    /// 全局键（仅内存生效；绝不写回配置文件）。被覆盖的全局键以 info
    /// 级别记录完整路径，例如 `privacy_mask_extension.regex.ExamApiKey`。
    pub fn load(project_root: &Path) -> CoreResult<Self> {
        let home = user_dir::home_dir()?;
        Self::load_with_home(project_root, &home)
    }

    /// Like [`load`](Self::load) with an explicit home directory, used by
    /// tests to avoid touching the real user home.
    /// 同 [`load`](Self::load)，但以显式指定的主目录代替真实用户主目录，
    /// 供测试避免触碰真实主目录。
    pub fn load_with_home(project_root: &Path, home: &Path) -> CoreResult<Self> {
        let global = read_table(&home.join(".ManualAid").join("config.toml"))?;
        let project = read_table(&project_root.join(".ManualAid").join("config.toml"))?;
        let (merged, overridden) = merge_tables(&global, &project);
        for key in overridden {
            log::info!("privacy mask extension: project value overrides global key `{key}`");
        }
        Ok(Self {
            regex: merged.regex,
            literal: merged.literal,
        })
    }
}

/// Read a config file's extension tables. A missing or empty file yields an
/// empty table; other I/O errors and TOML parse errors propagate.
/// 读取配置文件的扩展表。文件缺失或为空时返回空表；其他 I/O 错误与
/// TOML 解析错误向上传播。
fn read_table(path: &Path) -> CoreResult<ExtensionTable> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ExtensionTable::default());
        }
        Err(e) => return Err(CoreError::from(e)),
    };
    if content.trim().is_empty() {
        return Ok(ExtensionTable::default());
    }
    let config: ConfigFile = toml::from_str(&content)?;
    Ok(config.privacy_mask_extension)
}

/// Merge project over global per key (project wins). Returns the merged
/// table plus the full overridden keys (e.g.
/// `privacy_mask_extension.regex.ExamApiKey`) for logging and tests.
/// 按 key 将项目合并到全局之上（项目胜出）。返回合并后的表以及被覆盖的
/// 完整键列表（例如 `privacy_mask_extension.regex.ExamApiKey`），用于
/// 日志与测试。
fn merge_tables(
    global: &ExtensionTable,
    project: &ExtensionTable,
) -> (ExtensionTable, Vec<String>) {
    let mut merged = ExtensionTable {
        regex: global.regex.clone(),
        literal: global.literal.clone(),
    };
    let mut overridden = Vec::new();

    for (key, value) in &project.regex {
        if merged.regex.contains_key(key) {
            overridden.push(format!("privacy_mask_extension.regex.{key}"));
        }
        merged.regex.insert(key.clone(), value.clone());
    }
    for (key, value) in &project.literal {
        if merged.literal.contains_key(key) {
            overridden.push(format!("privacy_mask_extension.literal.{key}"));
        }
        merged.literal.insert(key.clone(), value.clone());
    }

    (merged, overridden)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
