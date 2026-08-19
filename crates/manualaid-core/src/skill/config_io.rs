use std::collections::HashMap;
use std::path::Path;

use toml::Table;
use toml_edit::{DocumentMut, Item, Table as TomlEditTable, Value as TomlEditValue};

use crate::error::{CoreError, CoreResult};
use crate::file_io;
use crate::manualaid_dir::DEFAULT_PROJECT_CONFIG_CONTENT;

/// The config-file key for a skill path: the absolute path with `\`
/// replaced by `/` so keys are identical across platforms.
/// 技能路径的配置键：绝对路径的 `\` 替换为 `/`，使键跨平台一致。
pub(super) fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Read the `[skill]` table of a config file into a path-keyed map.
///
/// A missing file yields an empty map. TOML syntax errors and non-boolean
/// values are `CoreError::Config` errors.
/// 读取配置文件中的 `[skill]` 表为路径键映射。
///
/// 文件缺失时返回空映射。TOML 语法错误与非布尔值返回
/// `CoreError::Config` 错误。
pub(super) fn read_enabled_map(config_path: &Path) -> CoreResult<HashMap<String, bool>> {
    file_io::with_file_lock(config_path, || {
        let content = match std::fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "cannot read skill config `{}`: {e}",
                    config_path.display()
                )));
            }
        };
        let table: Table = toml::from_str(&content)?;
        let mut enabled = HashMap::new();
        if let Some(toml::Value::Table(skill)) = table.get("skill") {
            for (key, value) in skill {
                match value {
                    toml::Value::Boolean(value) => {
                        enabled.insert(key.clone(), *value);
                    }
                    _ => {
                        return Err(CoreError::Config(format!(
                            "skill config entry `{key}` must be a boolean"
                        )));
                    }
                }
            }
        }
        Ok(enabled)
    })
}

/// Write a path-keyed map back into the `[skill]` table of a config file,
/// creating parent directories and the file itself when missing. Other
/// tables, comments and formatting in an existing file are preserved; a
/// missing or blank file is seeded from the default project template.
/// 将路径键映射写回配置文件的 `[skill]` 表，缺失时创建父目录与文件本身。
/// 已有文件中的其他配置节、注释与格式会被保留；文件缺失或空白时以
/// 默认项目模板为基底创建。
pub(super) fn write_enabled_map(
    config_path: &Path,
    enabled: &HashMap<String, bool>,
) -> CoreResult<()> {
    file_io::with_file_lock(config_path, || {
        let content = match std::fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "cannot read skill config `{}`: {e}",
                    config_path.display()
                )));
            }
        };

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::Io(format!(
                    "cannot create skill config directory `{}`: {e}",
                    parent.display()
                ))
            })?;
        }

        // Missing or blank files are seeded from the default project
        // template so the generated config always carries its comments.
        // 缺失或空白的配置文件以默认项目模板为基底，保证文件始终带说明注释。
        let mut doc = if content.trim().is_empty() {
            DEFAULT_PROJECT_CONFIG_CONTENT
                .parse::<DocumentMut>()
                .map_err(|e| CoreError::Config(e.to_string()))?
        } else {
            content
                .parse::<DocumentMut>()
                .map_err(|e| CoreError::Config(e.to_string()))?
        };

        let skill_table = match doc.as_table_mut().entry("skill") {
            toml_edit::Entry::Occupied(mut occupied) => {
                if occupied.get().as_table().is_none() {
                    occupied.insert(Item::Table(TomlEditTable::new()));
                }
                occupied
                    .into_mut()
                    .as_table_mut()
                    .expect("skill entry is a table")
            }
            toml_edit::Entry::Vacant(vacant) => vacant
                .insert(Item::Table(TomlEditTable::new()))
                .as_table_mut()
                .expect("just inserted a table"),
        };

        let stale: Vec<String> = skill_table
            .iter()
            .map(|(key, _)| key.to_string())
            .filter(|key| !enabled.contains_key(key))
            .collect();
        for key in stale {
            skill_table.remove(&key);
        }

        for (key, value) in enabled {
            let item: Item = TomlEditValue::from(*value).into();
            match skill_table.entry(key) {
                toml_edit::Entry::Occupied(mut occupied) => {
                    occupied.insert(item);
                }
                toml_edit::Entry::Vacant(vacant) => {
                    vacant.insert(item);
                }
            }
        }

        std::fs::write(config_path, doc.to_string()).map_err(CoreError::from)?;
        Ok(())
    })
}
