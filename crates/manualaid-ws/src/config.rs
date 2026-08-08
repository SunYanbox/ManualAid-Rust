//! Global + project configuration loading, merging and saving for the CLI
//! loop. Project values override global values; saving writes only the
//! project file, preserving unrelated tables.
//! CLI loop 的全局 + 项目配置加载、合并与保存。项目值覆盖全局值；保存
//! 只写项目文件，保留无关的配置表。

use std::path::{Path, PathBuf};

use manualaid_core::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

/// A validation issue found while loading a config file.
/// 加载配置文件时发现的验证问题。
#[derive(Debug, Clone)]
pub struct ConfigIssue {
    /// The config key that has an invalid value.
    /// 具有无效值的配置键。
    pub key: String,
    /// The invalid value.
    /// 无效值。
    pub value: String,
    /// List of valid values for this key.
    /// 该键的有效值列表。
    pub available_values: Vec<String>,
    /// Path to the config file containing the invalid value.
    /// 包含无效值的配置文件路径。
    pub path: PathBuf,
}

/// Raw on-disk shape of one config file. All fields are optional so a file
/// can carry only the sections the user configured.
/// 单个配置文件在磁盘上的原始形态。所有字段均为可选，文件可以只携带
/// 用户配置过的部分。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigFile {
    /// `[global]` section: language and tool-call format.
    /// `[global]` 节：语言与工具调用格式。
    pub global: GlobalSection,
    /// `[tools]` section: per-tool enable switches.
    /// `[tools]` 节：各工具启用开关。
    pub tools: ToolsSection,
    /// `[permissions]` section: whitelisted commands.
    /// `[permissions]` 节：白名单命令。
    pub permissions: PermissionsSection,
}

/// The `[global]` table of a config file.
/// 配置文件中的 `[global]` 表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSection {
    /// Interface language code (`en` or `zh-CN`).
    /// 界面语言代码（`en` 或 `zh-CN`）。
    pub lang: Option<String>,
    /// Tool-call format label (`auto`, `xml` or `json-codeblock`).
    /// 工具调用格式标签（`auto`、`xml` 或 `json-codeblock`）。
    pub tool_call_format: Option<String>,
    /// Maximum characters for result text copied to clipboard.
    /// 复制到剪贴板的结果文本最大字符数。
    pub max_result_chars: Option<usize>,
    /// Whether context files (AGENTS.md etc.) are loaded into the prompt.
    /// 是否将上下文文件（AGENTS.md 等）加载到提示词中。
    pub context_auto_load: Option<bool>,
}

/// The `[tools]` table of a config file.
/// 配置文件中的 `[tools]` 表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsSection {
    /// Whether the shell tool is enabled.
    /// Shell 工具是否启用。
    pub shell: Option<bool>,
    /// Whether the read tool is enabled.
    /// Read 工具是否启用。
    pub read: Option<bool>,
    /// Whether the edit tool is enabled.
    /// Edit 工具是否启用。
    pub edit: Option<bool>,
    /// Whether the write tool is enabled.
    /// Write 工具是否启用。
    pub write: Option<bool>,
    /// Whether the skill tool is enabled.
    /// Skill 工具是否启用。
    pub skill: Option<bool>,
}

/// The `[permissions]` table of a config file.
/// 配置文件中的 `[permissions]` 表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PermissionsSection {
    /// Shell commands that are pre-approved without user interaction.
    /// 无需用户交互即可执行的预批准 shell 命令。
    pub allow_commands: Option<Vec<String>>,
}

/// The merged runtime configuration used by the CLI loop.
/// CLI loop 使用的合并后运行时配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Interface language code (`en` or `zh-CN`).
    /// 界面语言代码（`en` 或 `zh-CN`）。
    pub lang: String,
    /// Tool-call format label (`auto`, `xml` or `json-codeblock`).
    /// 工具调用格式标签（`auto`、`xml` 或 `json-codeblock`）。
    pub tool_call_format: String,
    /// Whether the shell tool is enabled.
    /// Shell 工具是否启用。
    pub shell: bool,
    /// Whether the read tool is enabled.
    /// Read 工具是否启用。
    pub read: bool,
    /// Whether the edit tool is enabled.
    /// Edit 工具是否启用。
    pub edit: bool,
    /// Whether the write tool is enabled.
    /// Write 工具是否启用。
    pub write: bool,
    /// Whether the skill tool is enabled.
    /// Skill 工具是否启用。
    pub skill: bool,
    /// Whitelisted shell commands.
    /// 白名单 shell 命令。
    pub allow_commands: Vec<String>,
    /// Maximum characters for result text copied to clipboard.
    /// 复制到剪贴板的结果文本最大字符数。
    pub max_result_chars: usize,
    /// Whether context files (AGENTS.md etc.) are loaded into the prompt.
    /// 是否将上下文文件（AGENTS.md 等）加载到提示词中。
    pub context_auto_load: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lang: "en".to_string(),
            tool_call_format: "auto".to_string(),
            shell: true,
            read: true,
            edit: true,
            write: true,
            skill: true,
            allow_commands: Vec::new(),
            max_result_chars: 50_000,
            context_auto_load: true,
        }
    }
}

impl Config {
    /// Whether `lang` is a supported interface language.
    /// `lang` 是否为受支持的界面语言。
    pub fn is_valid_lang(lang: &str) -> bool {
        matches!(lang, "en" | "zh-CN")
    }

    /// Whether `tool_call_format` is a supported format label.
    /// `tool_call_format` 是否为受支持的格式标签。
    pub fn is_valid_format(format: &str) -> bool {
        manualaid_core::parser::RegistryMode::from_label(format).is_some()
    }

    /// The names of the enabled tools in canonical order.
    /// 已启用工具的名称（按规范顺序）。
    pub fn enabled_tool_names(&self) -> Vec<String> {
        manualaid_core::tools::all_tools()
            .iter()
            .filter(|tool| match tool {
                manualaid_core::tools::ToolKind::Shell => self.shell,
                manualaid_core::tools::ToolKind::Read => self.read,
                manualaid_core::tools::ToolKind::Edit => self.edit,
                manualaid_core::tools::ToolKind::Write => self.write,
                manualaid_core::tools::ToolKind::Skill => self.skill,
            })
            .map(|tool| tool.name().to_string())
            .collect()
    }
}

/// Load and merge the global and project config files. Project values
/// override global values; missing files contribute their defaults.
/// Returns the merged config and a list of validation issues for invalid
/// values found in the config files.
/// 加载并合并全局与项目配置文件。项目值覆盖全局值；缺失的文件按默认
/// 值处理。返回合并后的配置以及配置文件中发现的无效值验证问题列表。
pub fn load(project_root: &Path, home: &Path) -> CoreResult<(Config, Vec<ConfigIssue>)> {
    let global_path = home.join(".ManualAid").join("config.toml");
    let project_path = project_root.join(".ManualAid").join("config.toml");
    let global = read_config_file(&global_path)?;
    let project = read_config_file(&project_path)?;
    Ok(merge_with_issues(
        global,
        project,
        &global_path,
        &project_path,
    ))
}

/// Merge a raw global file with a raw project file into runtime config,
/// collecting validation issues for invalid values.
/// 将原始全局文件与原始项目文件合并为运行时配置，并收集无效值的验证问题。
fn merge_with_issues(
    global: ConfigFile,
    project: ConfigFile,
    global_path: &Path,
    project_path: &Path,
) -> (Config, Vec<ConfigIssue>) {
    let defaults = Config::default();
    let mut issues = Vec::new();
    let paths = ConfigPaths {
        global: global_path,
        project: project_path,
    };

    // Validate and merge lang
    let (lang, lang_issue) = resolve_string_with_validation(
        project.global.lang.as_deref(),
        global.global.lang.as_deref(),
        "lang",
        &defaults.lang,
        Config::is_valid_lang,
        || vec!["en".to_string(), "zh-CN".to_string()],
        &paths,
    );
    if let Some(issue) = lang_issue {
        issues.push(issue);
    }

    // Validate and merge tool_call_format
    let (tool_call_format, format_issue) = resolve_string_with_validation(
        project.global.tool_call_format.as_deref(),
        global.global.tool_call_format.as_deref(),
        "tool_call_format",
        &defaults.tool_call_format,
        Config::is_valid_format,
        || {
            manualaid_core::parser::RegistryMode::all_labels()
                .iter()
                .map(|s| s.to_string())
                .collect()
        },
        &paths,
    );
    if let Some(issue) = format_issue {
        issues.push(issue);
    }

    let config = Config {
        lang,
        tool_call_format,
        shell: project
            .tools
            .shell
            .or(global.tools.shell)
            .unwrap_or(defaults.shell),
        read: project
            .tools
            .read
            .or(global.tools.read)
            .unwrap_or(defaults.read),
        edit: project
            .tools
            .edit
            .or(global.tools.edit)
            .unwrap_or(defaults.edit),
        write: project
            .tools
            .write
            .or(global.tools.write)
            .unwrap_or(defaults.write),
        skill: project
            .tools
            .skill
            .or(global.tools.skill)
            .unwrap_or(defaults.skill),
        allow_commands: project
            .permissions
            .allow_commands
            .or(global.permissions.allow_commands)
            .unwrap_or_default(),
        max_result_chars: project
            .global
            .max_result_chars
            .or(global.global.max_result_chars)
            .unwrap_or(defaults.max_result_chars),
        context_auto_load: project
            .global
            .context_auto_load
            .or(global.global.context_auto_load)
            .unwrap_or(defaults.context_auto_load),
    };
    (config, issues)
}

/// Paths of the global and project config files, used to locate the source
/// of a validation issue. Grouping them keeps `resolve_string_with_validation`
/// within the clippy argument-count limit.
/// 全局与项目配置文件路径，用于定位验证问题的来源。将它们合并为一个参数
/// 以将 `resolve_string_with_validation` 的参数数量控制在 clippy 限制内。
struct ConfigPaths<'a> {
    global: &'a Path,
    project: &'a Path,
}

/// Resolve a string config value from project and global sources,
/// validating it against a predicate. If invalid, record a ConfigIssue
/// with available values and fall back to the default.
/// 从项目和全局来源解析字符串配置值，并用断言进行验证。如果无效，
/// 记录包含可用值的 ConfigIssue 并回退到默认值。
fn resolve_string_with_validation<F: Fn(&str) -> bool, V: Fn() -> Vec<String>>(
    project_val: Option<&str>,
    global_val: Option<&str>,
    key: &str,
    default: &str,
    is_valid: F,
    available: V,
    paths: &ConfigPaths<'_>,
) -> (String, Option<ConfigIssue>) {
    // Prefer project value if present and valid
    if let Some(value) = project_val {
        if is_valid(value) {
            return (value.to_string(), None);
        } else {
            let issue = ConfigIssue {
                key: key.to_string(),
                value: value.to_string(),
                available_values: available(),
                path: paths.project.to_path_buf(),
            };
            // Fall through to check global
            if let Some(global_value) = global_val {
                if is_valid(global_value) {
                    return (global_value.to_string(), Some(issue));
                }
                // Global also invalid - collect both?
                // For simplicity, we only record the first invalid (project)
                // and fall back to default
                return (default.to_string(), Some(issue));
            }
            return (default.to_string(), Some(issue));
        }
    }

    // No project value, check global
    if let Some(value) = global_val {
        if is_valid(value) {
            return (value.to_string(), None);
        } else {
            let issue = ConfigIssue {
                key: key.to_string(),
                value: value.to_string(),
                available_values: available(),
                path: paths.global.to_path_buf(),
            };
            return (default.to_string(), Some(issue));
        }
    }

    // Neither present
    (default.to_string(), None)
}

/// Read one config file; a missing file yields an empty [`ConfigFile`].
/// 读取一个配置文件；文件缺失时返回空的 [`ConfigFile`]。
fn read_config_file(path: &Path) -> CoreResult<ConfigFile> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ConfigFile::default()),
        Err(e) => {
            return Err(CoreError::Io(format!(
                "cannot read config `{}`: {e}",
                path.display()
            )));
        }
    };
    toml::from_str(&content)
        .map_err(|e| CoreError::Config(format!("invalid config `{}`: {e}", path.display())))
}

/// Read the project config file as an editable document, creating the
/// `.ManualAid` directory when missing; a missing file yields an empty doc.
/// 将项目配置文件读为可编辑文档；`.ManualAid` 目录缺失时创建，文件缺失
/// 时返回空文档。
fn project_doc(project_root: &Path) -> CoreResult<(PathBuf, toml_edit::DocumentMut)> {
    let path = project_root.join(".ManualAid").join("config.toml");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            CoreError::Io(format!(
                "cannot create config directory `{}`: {e}",
                parent.display()
            ))
        })?;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(CoreError::Io(format!(
                "cannot read config `{}`: {e}",
                path.display()
            )));
        }
    };

    let doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| CoreError::Config(e.to_string()))?;
    Ok((path, doc))
}

/// Persist the runtime config into the project config file, preserving
/// existing tables such as `[skill]` and `[privacy_mask_extension]`.
/// The global config file is never touched.
/// 将运行时配置持久化到项目配置文件，保留 `[skill]`、
/// `[privacy_mask_extension]` 等已有配置表。全局配置文件不会被触碰。
pub fn save_project(project_root: &Path, config: &Config) -> CoreResult<()> {
    let (path, mut doc) = project_doc(project_root)?;

    set_table_string(&mut doc, "global", "lang", &config.lang);
    set_table_string(
        &mut doc,
        "global",
        "tool_call_format",
        &config.tool_call_format,
    );
    set_table_int(
        &mut doc,
        "global",
        "max_result_chars",
        config.max_result_chars,
    );
    set_table_bool(
        &mut doc,
        "global",
        "context_auto_load",
        config.context_auto_load,
    );
    set_table_bool(&mut doc, "tools", "shell", config.shell);
    set_table_bool(&mut doc, "tools", "read", config.read);
    set_table_bool(&mut doc, "tools", "edit", config.edit);
    set_table_bool(&mut doc, "tools", "write", config.write);
    set_table_bool(&mut doc, "tools", "skill", config.skill);
    set_table_array(
        &mut doc,
        "permissions",
        "allow_commands",
        &config.allow_commands,
    );

    std::fs::write(&path, doc.to_string()).map_err(CoreError::from)?;
    Ok(())
}

/// Persist only `max_result_chars` into the project config file so the
/// effective limit is always visible and editable there; all other tables
/// are preserved. Creates the file when it does not exist.
/// 只把 `max_result_chars` 持久化到项目配置文件，使生效的限额始终在该
/// 文件中可见可改；其他配置表全部保留。文件不存在时创建。
pub fn save_max_result_chars(project_root: &Path, value: usize) -> CoreResult<()> {
    let (path, mut doc) = project_doc(project_root)?;
    set_table_int(&mut doc, "global", "max_result_chars", value);
    std::fs::write(&path, doc.to_string()).map_err(CoreError::from)?;
    Ok(())
}

/// Insert or update a string value in `[table]` of a TOML document.
/// 在 TOML 文档的 `[table]` 中插入或更新字符串值。
fn set_table_string(doc: &mut toml_edit::DocumentMut, table: &str, key: &str, value: &str) {
    table_mut(doc, table).insert(key, toml_edit::value(value));
}

/// Insert or update an integer value in `[table]` of a TOML document.
/// 在 TOML 文档的 `[table]` 中插入或更新整数值。
fn set_table_int(doc: &mut toml_edit::DocumentMut, table: &str, key: &str, value: usize) {
    table_mut(doc, table).insert(key, toml_edit::value(value as i64));
}

/// Insert or update a boolean value in `[table]` of a TOML document.
/// 在 TOML 文档的 `[table]` 中插入或更新布尔值。
fn set_table_bool(doc: &mut toml_edit::DocumentMut, table: &str, key: &str, value: bool) {
    table_mut(doc, table).insert(key, toml_edit::value(value));
}

/// Insert or update a string-array value in `[table]` of a TOML document.
/// 在 TOML 文档的 `[table]` 中插入或更新字符串数组值。
fn set_table_array(doc: &mut toml_edit::DocumentMut, table: &str, key: &str, values: &[String]) {
    let array: toml_edit::Array = values
        .iter()
        .map(|value| toml_edit::Value::from(value.as_str()))
        .collect();
    table_mut(doc, table).insert(key, toml_edit::value(array));
}

/// Get or create the named table inside the document root.
/// 获取或创建文档根内的命名表。
fn table_mut<'a>(doc: &'a mut toml_edit::DocumentMut, name: &str) -> &'a mut toml_edit::Table {
    let root = doc.as_table_mut();
    match root.entry(name) {
        toml_edit::Entry::Occupied(occupied) => occupied
            .into_mut()
            .as_table_mut()
            .expect("config table must be a table"),
        toml_edit::Entry::Vacant(vacant) => vacant
            .insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .expect("just inserted a table"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_prefers_project_values() {
        let global = ConfigFile {
            global: GlobalSection {
                lang: Some("en".into()),
                tool_call_format: Some("xml".into()),
                max_result_chars: Some(200_000),
                context_auto_load: Some(true),
            },
            tools: ToolsSection {
                shell: Some(true),
                ..Default::default()
            },
            permissions: PermissionsSection {
                allow_commands: Some(vec!["git status".into()]),
            },
        };
        let project = ConfigFile {
            global: GlobalSection {
                lang: Some("zh-CN".into()),
                max_result_chars: Some(100_000),
                ..Default::default()
            },
            tools: ToolsSection {
                shell: Some(false),
                ..Default::default()
            },
            permissions: PermissionsSection::default(),
        };
        let (config, issues) = merge_with_issues(
            global,
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.lang, "zh-CN");
        assert_eq!(config.tool_call_format, "xml");
        assert_eq!(config.max_result_chars, 100_000);
        assert!(!config.shell);
        assert_eq!(config.allow_commands, vec!["git status"]);
        assert!(issues.is_empty());
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let global = ConfigFile {
            global: GlobalSection {
                lang: Some("fr".into()),
                tool_call_format: Some("bogus".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            ConfigFile::default(),
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.lang, "en");
        assert_eq!(config.tool_call_format, "auto");
        assert_eq!(config.max_result_chars, 50_000);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].key, "lang");
        assert_eq!(issues[0].value, "fr");
        assert_eq!(issues[1].key, "tool_call_format");
        assert_eq!(issues[1].value, "bogus");
    }

    #[test]
    fn enabled_tool_names_follows_switches() {
        let config = Config {
            shell: false,
            ..Config::default()
        };
        let names = config.enabled_tool_names();
        assert!(!names.contains(&"shell".to_string()));
        assert!(names.contains(&"read".to_string()));
    }

    #[test]
    fn save_project_preserves_other_tables() {
        let root = std::env::temp_dir().join(format!("manualaid-ws-test-{}", std::process::id()));
        let dir = root.join(".ManualAid");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[skill]\n\"/a/b\" = true\n\n[privacy_mask_extension.literal]\nKey = \"v\"\n",
        )
        .unwrap();
        save_project(&root, &Config::default()).unwrap();
        let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(content.contains("[skill]"));
        assert!(content.contains("privacy_mask_extension"));
        assert!(content.contains("lang = \"en\""));
        assert!(content.contains("allow_commands"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_max_result_chars_writes_only_that_key() {
        let root =
            std::env::temp_dir().join(format!("manualaid-ws-test-{}-only", std::process::id()));
        let dir = root.join(".ManualAid");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[skill]\n\"/a/b\" = true\n\n[global]\nlang = \"zh-CN\"\n",
        )
        .unwrap();
        save_max_result_chars(&root, 30_000).unwrap();
        let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(content.contains("max_result_chars = 30000"));
        assert!(content.contains("lang = \"zh-CN\""));
        assert!(content.contains("[skill]"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_max_result_chars_creates_file_when_missing() {
        let root =
            std::env::temp_dir().join(format!("manualaid-ws-test-{}-new", std::process::id()));
        save_max_result_chars(&root, 50_000).unwrap();
        let content = std::fs::read_to_string(root.join(".ManualAid").join("config.toml")).unwrap();
        assert!(content.contains("[global]"));
        assert!(content.contains("max_result_chars = 50000"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_invalid_lang_falls_back_to_valid_global() {
        let global = ConfigFile {
            global: GlobalSection {
                lang: Some("en".into()),
                max_result_chars: Some(200_000),
                ..Default::default()
            },
            tools: ToolsSection {
                shell: Some(true),
                ..Default::default()
            },
            ..Default::default()
        };
        let project = ConfigFile {
            global: GlobalSection {
                lang: Some("fr".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.lang, "en");
        assert!(config.shell);
        assert_eq!(config.max_result_chars, 200_000);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, "lang");
        assert_eq!(issues[0].value, "fr");
        assert_eq!(issues[0].path, Path::new("project.toml"));
        assert_eq!(
            issues[0].available_values,
            vec!["en".to_string(), "zh-CN".to_string()]
        );
    }

    #[test]
    fn project_invalid_and_global_invalid_lang_use_default() {
        let global = ConfigFile {
            global: GlobalSection {
                lang: Some("de".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let project = ConfigFile {
            global: GlobalSection {
                lang: Some("fr".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.lang, "en");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, "lang");
        assert_eq!(issues[0].value, "fr");
        assert_eq!(issues[0].path, Path::new("project.toml"));
    }

    #[test]
    fn project_invalid_lang_without_global_uses_default() {
        let project = ConfigFile {
            global: GlobalSection {
                lang: Some("fr".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.lang, "en");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, Path::new("project.toml"));
    }

    #[test]
    fn invalid_project_format_falls_back_to_valid_global() {
        let global = ConfigFile {
            global: GlobalSection {
                tool_call_format: Some("xml".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let project = ConfigFile {
            global: GlobalSection {
                tool_call_format: Some("bogus".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.tool_call_format, "xml");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, "tool_call_format");
        assert_eq!(issues[0].value, "bogus");
        assert_eq!(issues[0].path, Path::new("project.toml"));
    }

    #[test]
    fn read_config_file_io_error_is_reported() {
        // A directory path fails with a non-NotFound error.
        let dir =
            std::env::temp_dir().join(format!("manualaid-ws-test-{}-read-io", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let err = read_config_file(&dir).unwrap_err();
        assert!(matches!(err, CoreError::Io(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn project_doc_rejects_invalid_toml() {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-test-{}-doc-invalid",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        std::fs::write(
            root.join(".ManualAid").join("config.toml"),
            "not [valid toml",
        )
        .unwrap();
        let err = project_doc(&root).unwrap_err();
        assert!(matches!(err, CoreError::Config(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_doc_io_error_is_reported() {
        // A directory at the config path fails with a non-NotFound error.
        let root =
            std::env::temp_dir().join(format!("manualaid-ws-test-{}-doc-io", std::process::id()));
        std::fs::create_dir_all(root.join(".ManualAid").join("config.toml")).unwrap();
        let err = project_doc(&root).unwrap_err();
        assert!(matches!(err, CoreError::Io(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn project_doc_create_dir_failure_is_reported() {
        // A regular file at the `.ManualAid` path makes create_dir_all fail.
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-test-{}-doc-mkdir",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(".ManualAid"), "occupied").unwrap();
        let err = project_doc(&root).unwrap_err();
        assert!(matches!(err, CoreError::Io(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    #[should_panic(expected = "config table must be a table")]
    fn save_project_panics_when_global_is_not_a_table() {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-test-{}-not-table",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
        std::fs::write(root.join(".ManualAid").join("config.toml"), "global = 5\n").unwrap();
        let _ = save_project(&root, &Config::default());
    }
}
