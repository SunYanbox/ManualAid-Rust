//! Core parser types: [`ParsedToolCall`], [`ParseError`] and the
//! [`ToolCallFormatParser`] trait shared by every wire format.
//! 核心解析器类型：[`ParsedToolCall`]、[`ParseError`]，以及每种线格式
//! 共享的 [`ToolCallFormatParser`] trait。

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::tool_set::EnabledToolSet;
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// A single tool call parsed from raw text input.
/// 从原始文本输入中解析出的单个工具调用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedToolCall {
    /// Normalized tool name (e.g. `"read"`, `"shell"`).
    /// 规范化的工具名称（例如 `"read"`、`"shell"`）。
    pub tool_name: String,
    /// Ordered key-value parameters extracted from the input. Values are
    /// kept as raw JSON values so they can be passed through to the
    /// executor in a type-flexible way.
    /// 从输入中提取的有序键值参数。值保留为原始 JSON 值，以便以类型
    /// 灵活的方式传递给执行器。
    pub params: IndexMap<String, Value>,
    /// Which format produced this parse result.
    /// 产生此解析结果的格式。
    pub format: ToolCallFormat,
    /// Source text span (character offset) for error reporting.
    /// 源文本跨度（字符偏移量），用于错误报告。
    pub source_offset: Option<usize>,
    /// Whether an unclosed parameter was detected (its captured text was
    /// discarded).
    /// 是否检测到参数未闭合（其捕获的文本被丢弃）。
    #[serde(default)]
    pub unclosed_param: bool,
    /// Whether the tool tag was unclosed to EOF while at least one valid
    /// parameter tag had been scanned.
    /// 在已扫描到至少一个合法参数标签的情况下，工具标签是否未闭合到 EOF。
    #[serde(default)]
    pub unclosed_tool: bool,
}

/// Errors that can occur during tool-call parsing.
/// 工具调用解析期间可能发生的错误。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseError {
    /// Human-readable error description.
    /// 人类可读的错误描述。
    pub message: String,
    /// Character offset of the error in the source text, if known.
    /// 错误在源文本中的字符偏移量（如果已知）。
    pub offset: Option<usize>,
    /// The format variant that produced this error.
    /// 产生此错误的格式变体。
    pub format: Option<ToolCallFormat>,
    /// Underlying cause details, if available.
    /// 底层原因详情（如果有）。
    pub cause: Option<String>,
}

impl ParseError {
    /// Create a new parse error.
    /// 创建一个新的解析错误。
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            offset: None,
            format: None,
            cause: None,
        }
    }

    /// Set the character offset.
    /// 设置字符偏移量。
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set the format variant.
    /// 设置格式变体。
    pub fn with_format(mut self, format: ToolCallFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Set the underlying cause.
    /// 设置底层原因。
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(offset) = self.offset {
            write!(f, " (at offset {offset})")?;
        }
        if let Some(cause) = &self.cause {
            write!(f, ": {cause}")?;
        }
        if let Some(format) = &self.format {
            write!(f, " [format: {format:?}]")?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseError {}

/// The result of a parse attempt: the recognized calls plus soft issues
/// (warnings) that did not abort parsing.
/// 一次解析尝试的结果：识别出的调用，以及未中断解析的软问题（警告）。
#[derive(Debug, Clone, Default)]
pub struct ParseOutcome {
    /// Zero or more recognized tool calls. Empty means the input is simply
    /// not in this format, rather than malformed.
    /// 零个或多个已识别的工具调用。为空表示输入有效但此格式不包含工具
    /// 调用（而非格式错误）。
    pub calls: Vec<ParsedToolCall>,
    /// Soft issues that did not abort parsing (e.g. a discarded parameter
    /// whose closing tag was missing).
    /// 未中断解析的软问题（例如因缺少闭合标签而被丢弃的参数）。
    pub warnings: Vec<String>,
}

/// A parser that extracts tool calls from raw text and renders standard
/// call templates for any tool in its own wire format.
/// 从原始文本中提取工具调用，并为其线格式中的任何工具渲染标准调用模板
/// 的解析器。
pub trait ToolCallFormatParser: Send + Sync {
    /// Stable identifier for this parser (e.g. `"xml"`, `"json-codeblock"`).
    /// 此解析器的稳定标识符（例如 `"xml"`、`"json-codeblock"`）。
    fn format_name(&self) -> &'static str;

    /// Attempt to parse tool calls from `input`.
    ///
    /// Only tool names and parameter names defined in `tools` are
    /// recognized; all other tags and keys are discarded. An empty `calls`
    /// vec means the input is simply not in this format, rather than
    /// malformed.
    /// 尝试从 `input` 中解析工具调用。
    ///
    /// 只识别 `tools` 中定义的工具名与参数名，其余标签与键一律丢弃。
    /// 空的 `calls` 列表表示输入有效但此格式不包含工具调用（而非格式
    /// 错误）。
    fn try_parse(&self, input: &str, tools: &EnabledToolSet) -> Result<ParseOutcome, ParseError>;

    /// Generate a standard call example for `tool` in this parser's own
    /// wire format, used by the prompt builder.
    /// 为此解析器自己的线格式中的 `tool` 生成标准调用示例，供提示词
    /// 构建器使用。
    fn tool_call_template(&self, tool: &ToolKind) -> String;
}
