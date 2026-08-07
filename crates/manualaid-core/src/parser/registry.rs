//! The format registry selects which parser handles incoming text: all
//! registered parsers in auto-detect order, or exactly one fixed format.
//! 格式注册表决定由哪个解析器处理传入文本：自动检测模式下按注册顺序
//! 尝试全部解析器，或固定使用某一种格式。

use std::sync::RwLock;

use indexmap::IndexMap;

use super::json_codeblock::JsonCodeblockParser;
use super::traits::{ParseError, ParsedToolCall, ToolCallFormatParser};
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
    /// The configuration label of this mode (`auto`, `xml`,
    /// `json-codeblock`).
    /// 此模式的配置标签（`auto`、`xml`、`json-codeblock`）。
    pub fn label(&self) -> &'static str {
        match self {
            Self::AutoDetect => "auto",
            Self::Fixed(ToolCallFormat::Xml) => "xml",
            Self::Fixed(ToolCallFormat::JsonCodeblock) => "json-codeblock",
        }
    }

    /// Resolve a configuration label into a mode.
    /// 将配置标签解析为模式。
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "auto" => Some(Self::AutoDetect),
            "xml" => Some(Self::Fixed(ToolCallFormat::Xml)),
            "json-codeblock" => Some(Self::Fixed(ToolCallFormat::JsonCodeblock)),
            _ => None,
        }
    }

    /// All labels in cycling order, used by the `/format` command.
    /// 按循环顺序排列的全部标签，供 `/format` 命令使用。
    pub fn all_labels() -> &'static [&'static str] {
        &["auto", "xml", "json-codeblock"]
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
}

impl FormatRegistry {
    /// Create a new registry pre-populated with the two built-in parsers.
    /// 创建一个新的注册表，预填充两个内置解析器。
    pub fn new() -> Self {
        let mut parsers: IndexMap<String, Box<dyn ToolCallFormatParser>> = IndexMap::new();
        parsers.insert("xml".to_string(), Box::new(XmlParser));
        parsers.insert("json-codeblock".to_string(), Box::new(JsonCodeblockParser));
        Self {
            parsers: RwLock::new(parsers),
            mode: RwLock::new(RegistryMode::AutoDetect),
        }
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
    pub fn parse(&self, input: &str) -> Result<Vec<ParsedToolCall>, ParseError> {
        match self.mode()? {
            RegistryMode::AutoDetect => self.parse_auto(input),
            RegistryMode::Fixed(format) => self.parse_with(format_name(format), input),
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
    pub fn parse_with(
        &self,
        format_name: &str,
        input: &str,
    ) -> Result<Vec<ParsedToolCall>, ParseError> {
        let guard = self
            .parsers
            .read()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        let parser = guard.get(format_name).ok_or_else(|| {
            ParseError::new(format!("No parser registered for format `{format_name}`"))
        })?;
        parser.try_parse(input)
    }

    /// Auto-detect: return the first non-empty parse result.
    /// 自动检测：返回第一个非空解析结果。
    fn parse_auto(&self, input: &str) -> Result<Vec<ParsedToolCall>, ParseError> {
        let guard = self
            .parsers
            .read()
            .map_err(|_| ParseError::new("Registry lock poisoned"))?;
        let mut last_error: Option<ParseError> = None;
        for parser in guard.values() {
            match parser.try_parse(input) {
                Ok(calls) if !calls.is_empty() => return Ok(calls),
                Ok(_) => {}
                Err(e) => last_error = Some(e),
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Ok(Vec::new()),
        }
    }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_detect_finds_xml_calls() {
        let registry = FormatRegistry::new();
        let calls = registry
            .parse("<read><file_path>/a.txt</file_path></read>")
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "read");
    }

    #[test]
    fn auto_detect_finds_json_calls() {
        let registry = FormatRegistry::new();
        let calls = registry
            .parse(
                "```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a.txt\"}}\n```",
            )
            .unwrap();
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn fixed_xml_mode_ignores_json_input() {
        let registry = FormatRegistry::new();
        registry
            .set_mode(RegistryMode::Fixed(ToolCallFormat::Xml))
            .unwrap();
        let calls = registry
            .parse("{\"tool_use\": \"read\", \"params\": {}}")
            .unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn mode_labels_round_trip() {
        for &label in RegistryMode::all_labels() {
            let mode = RegistryMode::from_label(label).unwrap();
            assert_eq!(mode.label(), label);
        }
        assert!(RegistryMode::from_label("bogus").is_none());
    }

    #[test]
    fn parse_with_unknown_format_is_an_error() {
        let registry = FormatRegistry::new();
        assert!(registry.parse_with("nonexistent", "input").is_err());
    }

    #[test]
    fn renders_template_for_current_mode() {
        let registry = FormatRegistry::new();
        registry
            .set_mode(RegistryMode::Fixed(ToolCallFormat::JsonCodeblock))
            .unwrap();
        let template = registry.render_tool_call_template(&ToolKind::Read).unwrap();
        assert!(template.contains("\"tool_use\": \"read\""));
    }

    #[test]
    fn registered_formats_contains_builtins() {
        let formats = FormatRegistry::new().registered_formats().unwrap();
        assert!(formats.contains(&"xml".to_string()));
        assert!(formats.contains(&"json-codeblock".to_string()));
    }
}
