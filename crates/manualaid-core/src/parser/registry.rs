//! The format registry selects which parser handles incoming text: all
//! registered parsers in auto-detect order, or exactly one fixed format.
//! 格式注册表决定由哪个解析器处理传入文本：自动检测模式下按注册顺序
//! 尝试全部解析器，或固定使用某一种格式。
//!
//! # Test notes
//! The "No parser registered for format" error branch of the fixed mode is
//! unreachable: [`RegistryMode::Fixed`] only accepts the three formats that
//! are always registered (`json-codeblock`, `invoke`, and `xml`). It stays
//! as a defensive guard for future formats and is not required to have high
//! test coverage.
//! # 测试说明
//! 固定模式下“未注册该格式的解析器”错误分支不可达：[`RegistryMode::Fixed`]
//! 只接受始终注册的三种格式（`json-codeblock`、`invoke` 与 `xml`）。该
//! 分支保留为面向未来格式的防御性检查，不要求高测试覆盖率。

use std::sync::{Arc, OnceLock, RwLock};

use indexmap::IndexMap;

use super::invoke::InvokeParser;
use super::json_codeblock::JsonCodeblockParser;
use super::tool_set::EnabledToolSet;
use super::traits::{ParseError, ParseOutcome, ToolCallFormatParser};
use super::xml::XmlParser;
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// Mode that controls how the registry applies parsers to incoming text.
/// 控制注册表如何将解析器应用于传入文本的模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryMode {
    /// Try every registered parser and return results from the first one
    /// that yields one or more tool calls.
    /// 尝试每个已注册的解析器，并从第一个产生一个或多个工具调用的
    /// 解析器返回结果。
    AutoDetect,
    /// Only use the parser associated with the given format.
    /// 仅使用与给定格式关联的解析器。
    Fixed(ToolCallFormat),
}

impl RegistryMode {
    /// The configuration label of this mode (`auto`,
    /// `json-codeblock`, `invoke`, `xml`).
    /// 此模式的配置标签（`auto`、`json-codeblock`、`invoke`、`xml`）。
    pub fn label(&self) -> &'static str {
        match self {
            Self::AutoDetect => "auto",
            Self::Fixed(ToolCallFormat::Xml) => "xml",
            Self::Fixed(ToolCallFormat::JsonCodeblock) => "json-codeblock",
            Self::Fixed(ToolCallFormat::Invoke) => "invoke",
        }
    }

    /// Resolve a configuration label into a mode.
    /// 将配置标签解析为模式。
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "auto" => Some(Self::AutoDetect),
            "xml" => Some(Self::Fixed(ToolCallFormat::Xml)),
            "json-codeblock" => Some(Self::Fixed(ToolCallFormat::JsonCodeblock)),
            "invoke" => Some(Self::Fixed(ToolCallFormat::Invoke)),
            _ => None,
        }
    }

    /// All labels in cycling order, used by the `/format` command.
    /// 按循环顺序排列的全部标签，供 `/format` 命令使用。
    pub fn all_labels() -> &'static [&'static str] {
        &["auto", "json-codeblock", "invoke", "xml"]
    }
}

/// Thread-safe registry of the built-in [`ToolCallFormatParser`]s.
/// 内置 [`ToolCallFormatParser`] 的线程安全注册表。
pub struct FormatRegistry {
    /// Parsers indexed by format name.
    /// 按格式名称索引的解析器。
    parsers: RwLock<IndexMap<String, Box<dyn ToolCallFormatParser>>>,
    /// The current registry mode.
    /// 当前注册表模式。
    mode: RwLock<RegistryMode>,
    /// Cache of the enabled-tool set: the fingerprint (canonical tool
    /// names) plus the shared set. `None` means never configured, in which
    /// case every tool is available.
    /// 可用工具集合的缓存：指纹（规范顺序的工具名）与共享集合。`None`
    /// 表示从未配置，此时全部工具可用。
    enabled_tools: RwLock<Option<(Vec<String>, Arc<EnabledToolSet>)>>,
}

impl FormatRegistry {
    /// Create a new registry pre-populated with the two built-in parsers.
    /// 创建一个新的注册表，预填充两个内置解析器。
    pub fn new() -> Self {
        let mut parsers: IndexMap<String, Box<dyn ToolCallFormatParser>> = IndexMap::new();
        parsers.insert("json-codeblock".to_string(), Box::new(JsonCodeblockParser));
        parsers.insert("invoke".to_string(), Box::new(InvokeParser));
        parsers.insert("xml".to_string(), Box::new(XmlParser));
        Self {
            parsers: RwLock::new(parsers),
            mode: RwLock::new(RegistryMode::AutoDetect),
            enabled_tools: RwLock::new(None),
        }
    }

    /// Set the available tools. The lookup structures are rebuilt only when
    /// the effective set (unknown names dropped, order normalized) differs
    /// from the cached fingerprint; an unchanged set reuses the cache.
    /// 设置可用工具。只有当实际集合（未知名称已丢弃、顺序已规范化）与
    /// 缓存指纹不同时才重建查找结构；集合不变时直接复用缓存。
    pub fn set_enabled_tools(&self, tool_names: &[String]) -> Result<(), ParseError> {
        let set = EnabledToolSet::from_names(tool_names);
        let fingerprint = set.tool_names();
        let mut guard = self.enabled_tools.write().map_err(|_| lock_poisoned())?;
        let changed = guard
            .as_ref()
            .is_none_or(|(cached, _)| cached != &fingerprint);
        if changed {
            *guard = Some((fingerprint, Arc::new(set)));
        }
        Ok(())
    }

    /// The cached enabled-tool set, or the process-wide default (every
    /// tool) when never configured.
    /// 缓存的可用工具集合；从未配置时返回进程级默认（全部工具）。
    fn enabled_tool_set(&self) -> Result<Arc<EnabledToolSet>, ParseError> {
        let guard = self.enabled_tools.read().map_err(|_| lock_poisoned())?;
        Ok(guard
            .as_ref()
            .map(|(_, set)| set.clone())
            .unwrap_or_else(default_tool_set))
    }

    /// Set the registry mode.
    /// 设置注册表模式。
    pub fn set_mode(&self, mode: RegistryMode) -> Result<(), ParseError> {
        let mut guard = self
            .mode
            .write()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        *guard = mode;
        Ok(())
    }

    /// Get the current registry mode.
    /// 获取当前注册表模式。
    pub fn mode(&self) -> Result<RegistryMode, ParseError> {
        let guard = self
            .mode
            .read()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        Ok(*guard)
    }

    /// List all registered parser names.
    /// 列出所有已注册的解析器名称。
    pub fn registered_formats(&self) -> Result<Vec<String>, ParseError> {
        let guard = self
            .parsers
            .read()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        Ok(guard.keys().cloned().collect())
    }

    /// Parse `input` using the current mode.
    ///
    /// In `AutoDetect` mode the first parser yielding one or more calls
    /// wins; an error is returned only when no parser yielded calls and at
    /// least one reported an error.
    /// 使用当前模式解析 `input`。
    ///
    /// `AutoDetect` 模式下第一个产生调用（非空）的解析器获胜；仅当所有
    /// 解析器都没有产生调用且至少一个报告错误时才返回错误。
    pub fn parse(&self, input: &str) -> Result<ParseOutcome, ParseError> {
        let tools = self.enabled_tool_set()?;
        match self.mode()? {
            RegistryMode::AutoDetect => self.parse_auto(input, &tools),
            RegistryMode::Fixed(format) => {
                self.parse_with_tools(format_name(format), input, &tools)
            }
        }
    }

    /// Render a tool call template using the parser associated with the
    /// current mode.
    /// 使用与当前模式关联的解析器渲染工具调用模板。
    pub fn render_tool_call_template(&self, tool: &ToolKind) -> Result<String, ParseError> {
        let guard = self
            .parsers
            .read()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        let parser: &dyn ToolCallFormatParser = match self.mode()? {
            RegistryMode::AutoDetect => guard
                .values()
                .next()
                .ok_or_else(|| ParseError::new("No parser registered"))?
                .as_ref(),
            RegistryMode::Fixed(format) => {
                let name = format_name(format);
                guard
                    .get(name)
                    .ok_or_else(|| {
                        ParseError::new(format!("No parser registered for format `{name}`"))
                    })?
                    .as_ref()
            }
        };
        Ok(parser.tool_call_template(tool))
    }

    /// Parse with one specific parser by name.
    /// 按名称使用特定的解析器解析。
    pub fn parse_with(&self, format_name: &str, input: &str) -> Result<ParseOutcome, ParseError> {
        let tools = self.enabled_tool_set()?;
        self.parse_with_tools(format_name, input, &tools)
    }

    /// Parse with one specific parser by name, using a given tool set.
    /// 按名称使用特定的解析器解析，并指定工具集合。
    fn parse_with_tools(
        &self,
        format_name: &str,
        input: &str,
        tools: &EnabledToolSet,
    ) -> Result<ParseOutcome, ParseError> {
        let guard = self.parsers.read().map_err(|_| lock_poisoned())?;
        let parser = guard.get(format_name).ok_or_else(|| {
            ParseError::new(format!("No parser registered for format `{format_name}`"))
        })?;
        parser.try_parse(input, tools)
    }

    /// Auto-detect: return the first non-empty parse result; warnings of
    /// parsers that yielded no calls are merged into the final outcome.
    /// 自动检测：返回第一个非空解析结果；未产生调用的解析器的警告会
    /// 合并进最终结果。
    fn parse_auto(&self, input: &str, tools: &EnabledToolSet) -> Result<ParseOutcome, ParseError> {
        let guard = self.parsers.read().map_err(|_| lock_poisoned())?;
        let mut last_error: Option<ParseError> = None;
        let mut warnings = Vec::new();
        for parser in guard.values() {
            match parser.try_parse(input, tools) {
                Ok(outcome) if !outcome.calls.is_empty() => return Ok(outcome),
                Ok(outcome) => warnings.extend(outcome.warnings),
                Err(e) => last_error = Some(e),
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Ok(ParseOutcome {
                calls: Vec::new(),
                warnings,
            }),
        }
    }
}

/// The process-wide default tool set (every built-in tool), built once.
/// 进程级默认工具集合（全部内置工具），只构建一次。
fn default_tool_set() -> Arc<EnabledToolSet> {
    static DEFAULT: OnceLock<Arc<EnabledToolSet>> = OnceLock::new();
    DEFAULT
        .get_or_init(|| Arc::new(EnabledToolSet::all()))
        .clone()
}

/// The error used when a registry lock is poisoned.
/// 注册表锁被污染时使用的错误。
fn lock_poisoned() -> ParseError {
    ParseError::new("Registry lock poisoned")
}

impl Default for FormatRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The registry key of a wire format.
/// 线格式对应的注册表键名。
fn format_name(format: ToolCallFormat) -> &'static str {
    match format {
        ToolCallFormat::Xml => "xml",
        ToolCallFormat::JsonCodeblock => "json-codeblock",
        ToolCallFormat::Invoke => "invoke",
    }
}

#[cfg(test)]
mod tests;
