//! Core tool types: the [`ToolKind`] enum that unifies definition and
//! execution, the [`ToolParam`] descriptor, and the unified [`ToolResult`].
//! 核心工具类型：统一定义与执行的 [`ToolKind`] 枚举、参数描述符
//! [`ToolParam`]，以及统一结果结构 [`ToolResult`]。

use serde::{Deserialize, Serialize};

use crate::audit::AuditDecision;

/// Supported tool-calling wire formats that templates can be generated for.
/// 支持的工具调用线格式，可为其生成模板。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolCallFormat {
    /// Home-grown XML format (angle-bracket tags).
    /// 自研 XML 格式（尖括号标签）。
    Xml,
    /// Anthropic-style JSON code block.
    /// Anthropic 风格的 JSON 代码块。
    #[serde(rename = "json-codeblock")]
    JsonCodeblock,
}

impl ToolCallFormat {
    /// All known variants, used for iterating during initialisation and
    /// configuration menus.
    /// 所有已知变体，用于初始化与配置菜单中的迭代。
    pub fn all() -> &'static [Self] {
        &[Self::Xml, Self::JsonCodeblock]
    }
}

/// Semantic classification of a parameter value — used by the audit layer
/// to decide which safety checks apply without hard-coding tool names.
/// 参数值的语义分类——由审计层用于决定应用哪些安全检查，
/// 而无需硬编码工具名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParamSemantic {
    /// No special semantics (default for most parameters).
    /// 无特殊语义（大多数参数的默认值）。
    None,
    /// File-system path (read) — requires workspace-boundary checks.
    /// 文件系统路径（读取）——需要工作区边界检查。
    ReadPath,
    /// File-system path (write) — additionally triggers write approval.
    /// 文件系统路径（写入）——额外触发写入批准。
    WritePath,
    /// Shell command string — requires whitelist matching.
    /// Shell 命令字符串——需要白名单匹配。
    Command,
    /// File content that will be written to disk — privacy handled by the
    /// sanitize/restore pipeline.
    /// 将写入磁盘的文件内容——隐私由脱敏/还原管线处理。
    Content,
}

impl ParamSemantic {
    /// Returns `true` for [`WritePath`](ParamSemantic::WritePath).
    /// 如果是 [`WritePath`](ParamSemantic::WritePath) 则返回 `true`。
    pub fn is_write(self) -> bool {
        matches!(self, Self::WritePath)
    }
}

/// Describes one parameter accepted by a tool. Descriptions are i18n keys
/// resolved through the `i18n` crate so the UI language can switch at
/// runtime without recompiling.
/// 描述工具接受的一个参数。描述以 i18n 键存储，通过 `i18n` crate 解析，
/// 使界面语言可以在运行时切换而无需重新编译。
#[derive(Debug, Clone)]
pub struct ToolParam {
    /// Canonical parameter name (used as the XML tag or JSON key).
    /// 规范参数名称（用作 XML 标签或 JSON 键）。
    pub name: &'static str,
    /// Type hint (e.g. `"string"`, `"integer"`, `"boolean"`).
    /// 类型提示（例如 `"string"`、`"integer"`、`"boolean"`）。
    pub kind: &'static str,
    /// i18n key of the human-readable parameter description.
    /// 参数人类可读描述的 i18n 键。
    pub description_key: &'static str,
    /// Whether the parameter is required or optional.
    /// 参数是必需的还是可选的。
    pub required: bool,
    /// Semantic classification for auditing.
    /// 用于审计的语义分类。
    pub semantic: ParamSemantic,
}

impl ToolParam {
    /// Create a new parameter with [`ParamSemantic::None`].
    /// 创建一个具有 [`ParamSemantic::None`] 的新参数。
    pub const fn new(
        name: &'static str,
        kind: &'static str,
        description_key: &'static str,
        required: bool,
    ) -> Self {
        Self {
            name,
            kind,
            description_key,
            required,
            semantic: ParamSemantic::None,
        }
    }

    /// Create a new parameter with an explicit semantic tag.
    /// 创建一个具有显式语义标签的新参数。
    pub const fn with_semantic(
        name: &'static str,
        kind: &'static str,
        description_key: &'static str,
        required: bool,
        semantic: ParamSemantic,
    ) -> Self {
        Self {
            name,
            kind,
            description_key,
            required,
            semantic,
        }
    }

    /// The localized description of this parameter.
    /// 此参数的本地化描述。
    pub fn description(&self) -> String {
        i18n::t_str(self.description_key)
    }
}

/// Every built-in tool: definition and execution are unified in one enum,
/// so the executor routes a parsed call to [`ToolKind::run`] directly and
/// the parser renders templates from [`ToolKind::parameters`].
/// 每个内置工具：定义与执行统一在一个枚举中，执行器将解析出的调用直接
/// 路由到 [`ToolKind::run`]，解析器从 [`ToolKind::parameters`] 渲染模板。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    /// Execute a shell command (`shell`).
    /// 执行 shell 命令（`shell`）。
    Shell,
    /// Read file contents (`read`).
    /// 读取文件内容（`read`）。
    Read,
    /// Exact string replacement in a file (`edit`).
    /// 文件中的精确字符串替换（`edit`）。
    Edit,
    /// Create or overwrite a file (`write`).
    /// 创建或覆盖文件（`write`）。
    Write,
    /// Load a skill body (`skill`).
    /// 加载技能正文（`skill`）。
    Skill,
}

impl ToolKind {
    /// Canonical tool identifier used in wire formats and routing.
    /// 线格式与路由使用的规范工具标识符。
    pub fn name(&self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Read => "read",
            Self::Edit => "edit",
            Self::Write => "write",
            Self::Skill => "skill",
        }
    }

    /// Resolve a tool by its canonical name.
    /// 按规范名称解析工具。
    pub fn from_name(name: &str) -> Option<Self> {
        super::all_tools()
            .iter()
            .copied()
            .find(|tool| tool.name() == name)
    }

    /// i18n key of the tool description.
    /// 工具描述的 i18n 键。
    pub fn description_key(&self) -> &'static str {
        match self {
            Self::Shell => "tool.shell.desc",
            Self::Read => "tool.read.desc",
            Self::Edit => "tool.edit.desc",
            Self::Write => "tool.write.desc",
            Self::Skill => "tool.skill.desc",
        }
    }

    /// The localized tool description.
    /// 工具的本地化描述。
    pub fn description(&self) -> String {
        i18n::t_str(self.description_key())
    }

    /// The parameters this tool accepts, in canonical order.
    /// 此工具接受的参数（按规范顺序）。
    pub fn parameters(&self) -> Vec<ToolParam> {
        match self {
            Self::Shell => vec![
                ToolParam::with_semantic(
                    "command",
                    "string",
                    "tool.shell.param.command.desc",
                    true,
                    ParamSemantic::Command,
                ),
                ToolParam::new(
                    "description",
                    "string",
                    "tool.shell.param.description.desc",
                    true,
                ),
                ToolParam::new("timeout", "integer", "tool.shell.param.timeout.desc", false),
            ],
            Self::Read => vec![
                ToolParam::with_semantic(
                    "file_path",
                    "string",
                    "tool.read.param.file_path.desc",
                    true,
                    ParamSemantic::ReadPath,
                ),
                ToolParam::new("offset", "integer", "tool.read.param.offset.desc", false),
                ToolParam::new("limit", "integer", "tool.read.param.limit.desc", false),
            ],
            Self::Edit => vec![
                ToolParam::with_semantic(
                    "file_path",
                    "string",
                    "tool.edit.param.file_path.desc",
                    true,
                    ParamSemantic::WritePath,
                ),
                ToolParam::with_semantic(
                    "old_string",
                    "string",
                    "tool.edit.param.old_string.desc",
                    true,
                    ParamSemantic::Content,
                ),
                ToolParam::with_semantic(
                    "new_string",
                    "string",
                    "tool.edit.param.new_string.desc",
                    true,
                    ParamSemantic::Content,
                ),
                ToolParam::new(
                    "replace_all",
                    "boolean",
                    "tool.edit.param.replace_all.desc",
                    false,
                ),
            ],
            Self::Write => vec![
                ToolParam::with_semantic(
                    "file_path",
                    "string",
                    "tool.write.param.file_path.desc",
                    true,
                    ParamSemantic::WritePath,
                ),
                ToolParam::with_semantic(
                    "content",
                    "string",
                    "tool.write.param.content.desc",
                    true,
                    ParamSemantic::Content,
                ),
            ],
            Self::Skill => vec![
                ToolParam::new("skill", "string", "tool.skill.param.skill.desc", true),
                ToolParam::new("args", "string", "tool.skill.param.args.desc", false),
            ],
        }
    }

    /// Whether this tool is conceptually read-only (no side effects).
    /// 此工具在概念上是否为只读（无副作用）。
    pub fn is_read_only(&self) -> bool {
        matches!(self, Self::Read | Self::Skill)
    }

    /// Execute this tool with the given parameter map.
    /// 使用给定参数映射执行此工具。
    pub async fn run(&self, params: &indexmap::IndexMap<String, serde_json::Value>) -> ToolResult {
        match self {
            Self::Shell => super::shell::run(params).await,
            Self::Read => super::read::run(params).await,
            Self::Edit => super::edit::run(params).await,
            Self::Write => super::write::run(params).await,
            Self::Skill => super::skill::run(params).await,
        }
    }
}

/// The unified result of executing one tool call through the pipeline.
/// 工具调用通过完整管线执行后的统一结果。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolResult {
    /// Canonical tool name (e.g. `"read"`, `"shell"`).
    /// 规范工具名称（例如 `"read"`、`"shell"`）。
    pub tool_name: String,
    /// Whether the execution completed without known errors.
    /// 执行是否在无已知错误的情况下完成。
    pub success: bool,
    /// Human-readable output (may be pre-formatted).
    /// 人类可读的输出（可能已预格式化）。
    pub output: String,
    /// Whether this tool is conceptually read-only (no side effects).
    /// 此工具在概念上是否为只读（无副作用）。
    pub read_only: bool,
    /// Whether the output was synthesised from an error/empty condition
    /// rather than produced by the tool itself.
    /// 输出是否由错误/空条件合成而非工具本身产生。
    pub is_fallback: bool,
    /// Audit decisions attached to this execution (empty = allowed).
    /// 附加到此执行的审计决策（空 = 已允许）。
    #[serde(default)]
    pub audit_decisions: Vec<(String, AuditDecision)>,
    /// Parameter summary (JSON, at most 75 chars) for distinguishing tool
    /// calls within a round.
    /// 参数摘要（JSON，至多 75 字符），用于区分一轮中的工具调用。
    #[serde(default)]
    pub params_summary: String,
}

impl ToolResult {
    /// Create a successful result.
    /// 创建一个成功结果。
    pub fn success(
        tool_name: impl Into<String>,
        output: impl Into<String>,
        read_only: bool,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            success: true,
            output: output.into(),
            read_only,
            is_fallback: false,
            audit_decisions: Vec::new(),
            params_summary: String::new(),
        }
    }

    /// Create a failed result.
    /// 创建一个失败结果。
    pub fn failure(tool_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            success: false,
            output: message.into(),
            read_only: false,
            is_fallback: true,
            audit_decisions: Vec::new(),
            params_summary: String::new(),
        }
    }

    /// Attach a parameter summary used to distinguish calls within a round.
    /// 附加参数摘要，用于区分同一轮中的工具调用。
    pub fn with_params_summary(mut self, summary: String) -> Self {
        self.params_summary = summary;
        self
    }
}

/// Maximum characters of a parameter summary.
/// 参数摘要的最大字符数。
const PARAMS_SUMMARY_MAX_CHARS: usize = 75;

/// Serialize a parameter map into a truncated JSON summary. The summary is
/// based on the raw parsed parameters (masked placeholders are not restored)
/// so sensitive data never leaks into result text.
/// 将参数映射序列化为截断的 JSON 摘要。摘要基于解析后的原始参数
/// （不还原掩码占位符），避免敏感数据进入结果文本。
pub fn params_summary_of(params: &indexmap::IndexMap<String, serde_json::Value>) -> String {
    let json = serde_json::to_string(params).unwrap_or_default();
    json.chars().take(PARAMS_SUMMARY_MAX_CHARS).collect()
}
