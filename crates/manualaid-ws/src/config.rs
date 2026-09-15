//! Global + project configuration loading, merging and saving for the CLI
//! loop. Project values override global values; saving writes only the
//! project file, preserving unrelated tables.
//! CLI loop 的全局 + 项目配置加载、合并与保存。项目值覆盖全局值；保存
//! 只写项目文件，保留无关的配置表。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use manualaid_core::error::{CoreError, CoreResult};
use manualaid_core::mcp::{McpServerConfig, McpTransportKind};
use serde::{Deserialize, Serialize};

/// The kind of a config issue, controlling how the CLI reports it.
/// 配置问题的种类，决定 CLI 的展示方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigIssueKind {
    /// The value of a known config key is invalid.
    /// 已知配置键的值无效。
    InvalidValue,
    /// A `[permissions].allow_commands` entry matches a blacklisted
    /// command and was ignored.
    /// `[permissions].allow_commands` 条目命中黑名单命令，已被忽略。
    DangerousAllowCommand,
    /// An `[[mcp.servers]]` entry is invalid and was skipped.
    /// `[[mcp.servers]]` 条目无效，已被跳过。
    InvalidMcpServer,
}

/// A validation issue found while loading a config file.
/// 加载配置文件时发现的验证问题。
#[derive(Debug, Clone)]
pub struct ConfigIssue {
    /// What kind of issue this is.
    /// 该问题所属的种类。
    pub kind: ConfigIssueKind,
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
    /// `[mcp]` section: declared MCP servers.
    /// `[mcp]` 节：已声明的 MCP 服务器。
    pub mcp: McpSection,
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
    /// Whether the todo_write tool is enabled.
    /// Todo_write 工具是否启用。
    pub todo_write: Option<bool>,
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

/// The `[mcp]` table of a config file.
/// 配置文件中的 `[mcp]` 表。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpSection {
    /// Servers declared under `[[mcp.servers]]`.
    /// `[[mcp.servers]]` 下声明的服务器。
    pub servers: Vec<McpServerEntry>,
}

/// One `[[mcp.servers]]` entry as declared on disk.
///
/// `transport` is kept as a raw string rather than [`McpTransportKind`] so an
/// unknown label is reported as a validation issue and only skips that entry,
/// instead of failing the whole config file during deserialization.
/// 磁盘上声明的一个 `[[mcp.servers]]` 条目。
///
/// `transport` 保持为原始字符串而非 [`McpTransportKind`]，使未知标签被报告为
/// 验证问题且只跳过该条目，而不是在反序列化阶段让整个配置文件失败。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServerEntry {
    /// Server name, unique among the entries of the chosen file.
    /// 服务器名，在所选用文件的条目中唯一。
    pub name: String,
    /// Transport label (`stdio` or `http`).
    /// 传输标签（`stdio` 或 `http`）。
    pub transport: String,
    /// Executable to spawn; required by the stdio transport.
    /// 要启动的可执行文件；stdio 传输必需。
    pub command: Option<String>,
    /// Arguments passed to `command`.
    /// 传给 `command` 的参数。
    pub args: Vec<String>,
    /// Extra environment variables for the spawned process.
    /// 为所启动进程附加的环境变量。
    pub env: std::collections::HashMap<String, String>,
    /// Remote endpoint URL; required by the http transport.
    /// 远端端点 URL；http 传输必需。
    pub url: Option<String>,
    /// Whether this server is connected and its tools injected.
    /// 是否连接此服务器并注入其工具。
    pub enabled: bool,
}

impl Default for McpServerEntry {
    fn default() -> Self {
        Self {
            name: String::new(),
            // A missing `transport` means stdio, the common local case, so a
            // minimal `[[mcp.servers]]` entry needs only `name` and `command`;
            // a value that is present but unrecognized is still reported as a
            // validation issue during merging.
            // 缺失 `transport` 即视为 stdio（常见的本地情形），使最小的
            // `[[mcp.servers]]` 条目只需 `name` 与 `command`；写出但无法识别
            // 的值仍会在合并阶段被报告为验证问题。
            transport: McpTransportKind::Stdio.label().to_string(),
            command: None,
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            url: None,
            enabled: true,
        }
    }
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
    /// Whether the todo_write tool is enabled.
    /// Todo_write 工具是否启用。
    pub todo_write: bool,
    /// Whitelisted shell commands.
    /// 白名单 shell 命令。
    pub allow_commands: Vec<String>,
    /// Maximum characters for result text copied to clipboard.
    /// 复制到剪贴板的结果文本最大字符数。
    pub max_result_chars: usize,
    /// Whether context files (AGENTS.md etc.) are loaded into the prompt.
    /// 是否将上下文文件（AGENTS.md 等）加载到提示词中。
    pub context_auto_load: bool,
    /// MCP servers from the effective config file, in declaration order;
    /// invalid entries have already been dropped.
    /// 来自生效配置文件的 MCP 服务器，按声明顺序；无效条目已被丢弃。
    pub mcp_servers: Vec<McpServerConfig>,
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
            todo_write: true,
            allow_commands: Vec::new(),
            max_result_chars: 50_000,
            context_auto_load: true,
            mcp_servers: Vec::new(),
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

    /// The names of the enabled tools in canonical order: built-ins first,
    /// then the tools contributed by enabled MCP servers.
    /// 已启用工具的名称（按规范顺序）：内置工具在前，随后是已启用 MCP 服务器
    /// 贡献的工具。
    ///
    /// # Description
    /// MCP tools are switched on per server rather than per tool, so the
    /// `[tools]` flags never apply to them; an enabled server contributes
    /// every tool it exposes.
    /// # 描述
    /// MCP 工具按服务器而非按工具开关，因此 `[tools]` 开关对它们不适用；
    /// 已启用的服务器贡献其暴露的全部工具。
    pub fn enabled_tool_names(&self) -> Vec<String> {
        let mut names: Vec<String> = manualaid_core::tools::all_tools()
            .iter()
            .filter(|tool| match tool {
                manualaid_core::tools::ToolKind::Shell => self.shell,
                manualaid_core::tools::ToolKind::Read => self.read,
                manualaid_core::tools::ToolKind::Edit => self.edit,
                manualaid_core::tools::ToolKind::Write => self.write,
                manualaid_core::tools::ToolKind::Skill => self.skill,
                manualaid_core::tools::ToolKind::TodoWrite => self.todo_write,
            })
            .map(|tool| tool.name().to_string())
            .collect();
        names.extend(
            manualaid_core::mcp::enabled_tools()
                .into_iter()
                .map(|tool| tool.exposed_name),
        );
        names
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
        todo_write: project
            .tools
            .todo_write
            .or(global.tools.todo_write)
            .unwrap_or(defaults.todo_write),
        allow_commands: {
            let (allow_commands, allow_issues) = merge_allow_commands(
                project.permissions.allow_commands,
                global.permissions.allow_commands,
                project_path,
                global_path,
            );
            issues.extend(allow_issues);
            allow_commands
        },
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
        mcp_servers: {
            let (servers, mcp_issues) = merge_mcp_servers(
                project.mcp.servers,
                global.mcp.servers,
                project_path,
                global_path,
            );
            issues.extend(mcp_issues);
            servers
        },
    };
    (config, issues)
}

/// Resolve the effective `[permissions].allow_commands` list (project wins,
/// falling back to global) and drop entries that match a blacklisted
/// command. Each dropped entry becomes a `ConfigIssue` so the CLI can warn
/// the user that the dangerous rule was ignored.
/// 解析生效的 `[permissions].allow_commands` 列表（项目优先，回退全局），
/// 并丢弃命中黑名单命令的条目。每个被丢弃的条目生成一个 `ConfigIssue`，
/// 供 CLI 警告用户该危险规则已被忽略。
fn merge_allow_commands(
    project: Option<Vec<String>>,
    global: Option<Vec<String>>,
    project_path: &Path,
    global_path: &Path,
) -> (Vec<String>, Vec<ConfigIssue>) {
    let (source, path) = match (project, global) {
        (Some(commands), _) => (commands, project_path),
        (None, Some(commands)) => (commands, global_path),
        (None, None) => return (Vec::new(), Vec::new()),
    };
    let (kept, ignored) = manualaid_core::audit::sanitize_allow_commands(source);
    let issues = ignored
        .into_iter()
        .map(|command| ConfigIssue {
            kind: ConfigIssueKind::DangerousAllowCommand,
            key: "permissions.allow_commands".to_string(),
            value: command,
            available_values: Vec::new(),
            path: path.to_path_buf(),
        })
        .collect();
    (kept, issues)
}

/// Resolve the effective MCP server list and drop invalid entries.
///
/// # Description
/// Project servers win as a whole when the project declares any, otherwise the
/// global list is used; the two are never mixed, so a project cannot
/// accidentally half-override a global declaration. Every rejected entry
/// becomes a [`ConfigIssueKind::InvalidMcpServer`] issue carrying the reason,
/// so the CLI can tell the user exactly which declaration was skipped.
/// 解析生效的 MCP 服务器列表并丢弃无效条目。
///
/// # 描述
/// 项目声明了任何服务器时整体采用项目列表，否则采用全局列表；两者绝不混用，
/// 因此项目不会意外地只覆盖全局声明的一部分。每个被拒绝的条目生成一个
/// [`ConfigIssueKind::InvalidMcpServer`] 问题并附上原因，使 CLI 能准确告知
/// 用户哪条声明被跳过。
fn merge_mcp_servers(
    project: Vec<McpServerEntry>,
    global: Vec<McpServerEntry>,
    project_path: &Path,
    global_path: &Path,
) -> (Vec<McpServerConfig>, Vec<ConfigIssue>) {
    let (entries, path) = if !project.is_empty() {
        (project, project_path)
    } else if !global.is_empty() {
        (global, global_path)
    } else {
        return (Vec::new(), Vec::new());
    };

    let mut issues = Vec::new();
    let mut servers = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (index, entry) in entries.into_iter().enumerate() {
        let key = format!("mcp.servers[{index}]");

        // An unrecognized label falls back to stdio for the remaining checks,
        // so one typo cannot mask a second, independent problem in the same
        // entry.
        // 无法识别的标签在后续检查中回退为 stdio，使单个拼写错误不会掩盖同
        // 一条目中另一个独立问题。
        let transport = match McpTransportKind::from_label(entry.transport.trim()) {
            Some(transport) => transport,
            None => {
                issues.push(mcp_issue(
                    path,
                    &format!("{key}.transport"),
                    &entry.transport,
                    transport_labels(),
                ));
                McpTransportKind::Stdio
            }
        };

        // Deduplicate on the trimmed name, which is also what gets stored, so
        // `"a"` and `" a"` cannot both pass as distinct servers.
        // 以 trim 后的名字（也是最终存储的名字）去重，使 `"a"` 与 `" a"` 不会
        // 都被当作不同的服务器放行。
        let name = entry.name.trim().to_string();
        if name.is_empty() {
            issues.push(mcp_issue(
                path,
                &format!("{key}.name"),
                &entry.name,
                Vec::new(),
            ));
            continue;
        }
        if !seen.insert(name.clone()) {
            issues.push(mcp_issue(
                path,
                &format!("{key}.name"),
                &entry.name,
                Vec::new(),
            ));
            continue;
        }
        match transport {
            McpTransportKind::Stdio if is_blank(entry.command.as_deref()) => {
                issues.push(mcp_issue(
                    path,
                    &format!("{key}.command"),
                    entry.command.as_deref().unwrap_or_default(),
                    Vec::new(),
                ));
                continue;
            }
            McpTransportKind::Http if is_blank(entry.url.as_deref()) => {
                issues.push(mcp_issue(
                    path,
                    &format!("{key}.url"),
                    entry.url.as_deref().unwrap_or_default(),
                    Vec::new(),
                ));
                continue;
            }
            _ => {}
        }

        servers.push(McpServerConfig {
            name,
            transport,
            command: entry.command,
            args: entry.args,
            env: entry.env,
            url: entry.url,
            enabled: entry.enabled,
        });
    }

    (servers, issues)
}

/// Whether an optional string is absent or blank.
/// 可选字符串是否缺失或为空白。
fn is_blank(value: Option<&str>) -> bool {
    value.is_none_or(|value| value.trim().is_empty())
}

/// Build an issue describing one skipped `[[mcp.servers]]` entry.
/// 构造描述某个被跳过的 `[[mcp.servers]]` 条目的问题。
fn mcp_issue(path: &Path, key: &str, value: &str, available_values: Vec<String>) -> ConfigIssue {
    ConfigIssue {
        kind: ConfigIssueKind::InvalidMcpServer,
        key: key.to_string(),
        value: value.to_string(),
        available_values,
        path: path.to_path_buf(),
    }
}

/// The transport labels accepted in `[[mcp.servers]].transport`.
/// `[[mcp.servers]].transport` 接受的传输标签。
fn transport_labels() -> Vec<String> {
    [McpTransportKind::Stdio, McpTransportKind::Http]
        .iter()
        .map(|kind| kind.label().to_string())
        .collect()
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
                kind: ConfigIssueKind::InvalidValue,
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
                kind: ConfigIssueKind::InvalidValue,
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

/// Public variant of the internal config reader for use by CLI diagnostics.
/// Returns `Ok(ConfigFile::default())` when the file is missing so callers
/// never need to distinguish "not yet initialized" from "reads correctly".
/// 供 CLI 诊断使用的配置文件读取公开变体。
/// 文件缺失时返回 `Ok(ConfigFile::default())`，调用方无需区分"尚未初始化"
/// 与"读取正常"。
pub fn read_config_file_at(path: &Path) -> CoreResult<ConfigFile> {
    read_config_file(path)
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
    set_table_bool(&mut doc, "tools", "todo_write", config.todo_write);
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

/// Persist the declared MCP servers into the project config file under
/// `[[mcp.servers]]`, preserving every other table. The global config file is
/// never touched. An empty list removes the `[mcp]` table so a cleared
/// configuration does not leave a stale declaration behind.
/// 将已声明的 MCP 服务器持久化到项目配置文件的 `[[mcp.servers]]` 下，保留其他
/// 所有配置表。全局配置文件不会被触碰。列表为空时移除 `[mcp]` 表，使清空后的
/// 配置不会留下陈旧声明。
pub fn save_mcp_servers(project_root: &Path, servers: &[McpServerEntry]) -> CoreResult<()> {
    let (path, mut doc) = project_doc(project_root)?;
    if servers.is_empty() {
        doc.as_table_mut().remove("mcp");
    } else {
        let mut array = toml_edit::ArrayOfTables::new();
        for server in servers {
            array.push(server_entry_table(server));
        }
        let mut mcp = toml_edit::Table::new();
        mcp.insert("servers", toml_edit::Item::ArrayOfTables(array));
        doc.as_table_mut()
            .insert("mcp", toml_edit::Item::Table(mcp));
    }
    std::fs::write(&path, doc.to_string()).map_err(CoreError::from)?;
    Ok(())
}

/// Render one [`McpServerEntry`] as a TOML table, omitting empty optionals so
/// the file keeps carrying only the fields the user actually declared.
/// 将单个 [`McpServerEntry`] 渲染为 TOML 表，省略空的可选项，使文件只携带
/// 用户确实声明过的字段。
fn server_entry_table(server: &McpServerEntry) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    table.insert("name", toml_edit::value(server.name.as_str()));
    table.insert("transport", toml_edit::value(server.transport.as_str()));
    if let Some(command) = &server.command {
        table.insert("command", toml_edit::value(command.as_str()));
    }
    if !server.args.is_empty() {
        let args: toml_edit::Array = server
            .args
            .iter()
            .map(|arg| toml_edit::Value::from(arg.as_str()))
            .collect();
        table.insert("args", toml_edit::value(args));
    }
    if !server.env.is_empty() {
        // Sorted so repeated saves produce the same file; `HashMap` iteration
        // order does not.
        // 排序后写出，使多次保存产生同一文件；`HashMap` 的迭代顺序不稳定。
        let mut keys: Vec<&String> = server.env.keys().collect();
        keys.sort();
        let mut env = toml_edit::Table::new();
        for key in keys {
            env.insert(key, toml_edit::value(server.env[key].as_str()));
        }
        table.insert("env", toml_edit::Item::Table(env));
    }
    if let Some(url) = &server.url {
        table.insert("url", toml_edit::value(url.as_str()));
    }
    table.insert("enabled", toml_edit::value(server.enabled));
    table
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
            mcp: McpSection::default(),
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
            mcp: McpSection::default(),
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

    #[test]
    fn dangerous_allow_commands_are_dropped_with_issues() {
        let project = ConfigFile {
            permissions: PermissionsSection {
                allow_commands: Some(vec![
                    "git log *".into(),
                    "rm *".into(),
                    "*".into(),
                    "gh pr view *".into(),
                ]),
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.allow_commands, vec!["git log *", "gh pr view *"]);
        assert_eq!(issues.len(), 2);
        assert!(
            issues
                .iter()
                .all(|issue| issue.kind == ConfigIssueKind::DangerousAllowCommand)
        );
        assert_eq!(issues[0].value, "rm *");
        assert_eq!(issues[1].value, "*");
        assert_eq!(issues[0].key, "permissions.allow_commands");
        assert_eq!(issues[0].path, Path::new("project.toml"));
    }

    #[test]
    fn dangerous_allow_commands_from_global_are_dropped_with_issues() {
        let global = ConfigFile {
            permissions: PermissionsSection {
                allow_commands: Some(vec!["rm -rf /".into()]),
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            ConfigFile::default(),
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert!(config.allow_commands.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, ConfigIssueKind::DangerousAllowCommand);
        assert_eq!(issues[0].value, "rm -rf /");
        assert_eq!(issues[0].path, Path::new("global.toml"));
    }

    #[test]
    fn safe_allow_commands_produce_no_issues() {
        let project = ConfigFile {
            permissions: PermissionsSection {
                allow_commands: Some(vec!["git log *".into(), "git status".into()]),
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.allow_commands, vec!["git log *", "git status"]);
        assert!(issues.is_empty());
    }

    #[test]
    fn mcp_project_servers_replace_global_servers() {
        let global = ConfigFile {
            mcp: McpSection {
                servers: vec![McpServerEntry {
                    name: "global-server".into(),
                    command: Some("global-cmd".into()),
                    ..Default::default()
                }],
            },
            ..Default::default()
        };
        let project = ConfigFile {
            mcp: McpSection {
                servers: vec![McpServerEntry {
                    name: "project-server".into(),
                    command: Some("project-cmd".into()),
                    ..Default::default()
                }],
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].name, "project-server");
        assert_eq!(config.mcp_servers[0].transport, McpTransportKind::Stdio);
        assert!(issues.is_empty());
    }

    #[test]
    fn mcp_servers_fall_back_to_global_when_project_declares_none() {
        let global = ConfigFile {
            mcp: McpSection {
                servers: vec![McpServerEntry {
                    name: "from-global".into(),
                    command: Some("cmd".into()),
                    enabled: false,
                    ..Default::default()
                }],
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            global,
            ConfigFile::default(),
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].name, "from-global");
        assert!(!config.mcp_servers[0].enabled);
        assert!(issues.is_empty());
    }

    #[test]
    fn mcp_servers_are_empty_when_unconfigured() {
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            ConfigFile::default(),
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert!(config.mcp_servers.is_empty());
        assert!(issues.is_empty());
    }

    #[test]
    fn invalid_mcp_entries_are_skipped_with_issues() {
        let project = ConfigFile {
            mcp: McpSection {
                servers: vec![
                    McpServerEntry {
                        name: "   ".into(),
                        command: Some("cmd".into()),
                        ..Default::default()
                    },
                    McpServerEntry {
                        name: "no-command".into(),
                        ..Default::default()
                    },
                    McpServerEntry {
                        name: "no-url".into(),
                        transport: "http".into(),
                        ..Default::default()
                    },
                    McpServerEntry {
                        name: "duplicate".into(),
                        command: Some("first".into()),
                        ..Default::default()
                    },
                    McpServerEntry {
                        name: "duplicate".into(),
                        command: Some("second".into()),
                        ..Default::default()
                    },
                    McpServerEntry {
                        name: "kept".into(),
                        command: Some("cmd".into()),
                        ..Default::default()
                    },
                ],
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        let names: Vec<&str> = config
            .mcp_servers
            .iter()
            .map(|server| server.name.as_str())
            .collect();
        assert_eq!(names, vec!["duplicate", "kept"]);
        assert_eq!(issues.len(), 4);
        assert!(
            issues
                .iter()
                .all(|issue| issue.kind == ConfigIssueKind::InvalidMcpServer)
        );
        assert_eq!(issues[0].key, "mcp.servers[0].name");
        assert_eq!(issues[1].key, "mcp.servers[1].command");
        assert_eq!(issues[2].key, "mcp.servers[2].url");
        assert_eq!(issues[3].key, "mcp.servers[4].name");
        assert_eq!(issues[0].path, Path::new("project.toml"));
    }

    #[test]
    fn unknown_mcp_transport_falls_back_to_stdio_with_issue() {
        let project = ConfigFile {
            mcp: McpSection {
                servers: vec![McpServerEntry {
                    name: "remote".into(),
                    transport: "sse".into(),
                    command: Some("cmd".into()),
                    ..Default::default()
                }],
            },
            ..Default::default()
        };
        let (config, issues) = merge_with_issues(
            ConfigFile::default(),
            project,
            Path::new("global.toml"),
            Path::new("project.toml"),
        );
        assert_eq!(config.mcp_servers.len(), 1);
        assert_eq!(config.mcp_servers[0].transport, McpTransportKind::Stdio);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, "mcp.servers[0].transport");
        assert_eq!(issues[0].value, "sse");
        assert_eq!(
            issues[0].available_values,
            vec!["stdio".to_string(), "http".to_string()]
        );
    }

    #[test]
    fn save_mcp_servers_round_trips_and_preserves_other_tables() {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-test-{}-mcp-round-trip",
            std::process::id()
        ));
        let dir = root.join(".ManualAid");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[skill]\n\"/a/b\" = true\n\n[global]\nlang = \"zh-CN\"\n",
        )
        .unwrap();

        let servers = vec![
            McpServerEntry {
                name: "filesystem".into(),
                command: Some("npx".into()),
                args: vec!["-y".into(), "server-filesystem".into()],
                env: std::collections::HashMap::from([
                    ("ZED".to_string(), "z".to_string()),
                    ("ALPHA".to_string(), "a".to_string()),
                ]),
                ..Default::default()
            },
            McpServerEntry {
                name: "remote".into(),
                transport: "http".into(),
                url: Some("https://example.com/mcp".into()),
                enabled: false,
                ..Default::default()
            },
        ];
        save_mcp_servers(&root, &servers).unwrap();

        let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(content.contains("[[mcp.servers]]"));
        assert!(content.contains("[skill]"));
        assert!(content.contains("lang = \"zh-CN\""));
        // Environment keys are written sorted so repeated saves stay stable.
        // 环境变量键按排序写出，使多次保存结果稳定。
        assert!(content.find("ALPHA").unwrap() < content.find("ZED").unwrap());

        let parsed = read_config_file(&dir.join("config.toml")).unwrap();
        assert_eq!(parsed.mcp.servers.len(), 2);
        assert_eq!(parsed.mcp.servers[0].name, "filesystem");
        assert_eq!(parsed.mcp.servers[0].transport, "stdio");
        assert_eq!(parsed.mcp.servers[0].args, vec!["-y", "server-filesystem"]);
        assert_eq!(parsed.mcp.servers[0].env.len(), 2);
        assert_eq!(parsed.mcp.servers[1].transport, "http");
        assert_eq!(
            parsed.mcp.servers[1].url.as_deref(),
            Some("https://example.com/mcp")
        );
        assert!(!parsed.mcp.servers[1].enabled);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn save_mcp_servers_empty_list_removes_table() {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-test-{}-mcp-clear",
            std::process::id()
        ));
        let dir = root.join(".ManualAid");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            "[global]\nlang = \"en\"\n\n[[mcp.servers]]\nname = \"old\"\ncommand = \"cmd\"\n",
        )
        .unwrap();

        save_mcp_servers(&root, &[]).unwrap();

        let content = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(!content.contains("mcp"));
        assert!(content.contains("lang = \"en\""));
        let _ = std::fs::remove_dir_all(&root);
    }
}
