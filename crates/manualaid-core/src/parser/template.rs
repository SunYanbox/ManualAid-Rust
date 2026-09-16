//! A borrowed view over a tool's shape, shared by the built-in
//! [`ToolKind`] enum and runtime-discovered MCP tools.
//! 工具形态的借用视图，由内置 [`ToolKind`] 枚举与运行时发现的 MCP 工具
//! 共用。
//!
//! # Description
//! Template rendering only needs a tool's name and the name/kind/requiredness
//! of its parameters. Expressing that as a view keeps the parsers unaware of
//! where a tool came from, and lets [`ToolKind::parameters`] keep returning an
//! owned `Vec` instead of a borrowed slice.
//! # 描述
//! 模板渲染只需要工具名与其参数的名称、类型与必要性。把这个需求表达为
//! 视图，既让解析器不感知工具的来源，也让 [`ToolKind::parameters`] 继续返回
//! 拥有所有权的 `Vec` 而非借用切片。

use crate::mcp::McpTool;
use crate::tools::ToolKind;

/// One parameter as seen by a template renderer.
/// 模板渲染器所见的单个参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolTemplateParam<'a> {
    /// Parameter name, used as the XML tag or JSON key.
    /// 参数名称，用作 XML 标签或 JSON 键。
    pub name: &'a str,
    /// Type hint (`"string"`, `"integer"`, ...).
    /// 类型提示（`"string"`、`"integer"` 等）。
    pub kind: &'a str,
    /// Whether the parameter is required.
    /// 参数是否必需。
    pub required: bool,
}

/// A tool as seen by a template renderer.
/// 模板渲染器所见的工具。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTemplate<'a> {
    /// Name to render the call under.
    /// 渲染调用时使用的名称。
    pub name: &'a str,
    /// Parameters in the order they should be rendered.
    /// 参数，按渲染顺序排列。
    pub params: Vec<ToolTemplateParam<'a>>,
}

impl ToolTemplate<'static> {
    /// Build the view for a built-in tool.
    ///
    /// The names and kinds of built-in parameters are `&'static str`, so the
    /// resulting view borrows nothing from the caller.
    /// 为内置工具构建视图。
    ///
    /// 内置参数的名称与类型都是 `&'static str`，因此所得视图不借用调用方的
    /// 任何数据。
    pub fn from_kind(tool: ToolKind) -> Self {
        Self {
            name: tool.name(),
            params: tool
                .parameters()
                .into_iter()
                .map(|param| ToolTemplateParam {
                    name: param.name,
                    kind: param.kind,
                    required: param.required,
                })
                .collect(),
        }
    }
}

impl<'a> ToolTemplate<'a> {
    /// Build the view for a discovered MCP tool, borrowing its metadata.
    /// 为已发现的 MCP 工具构建视图，借用其元数据。
    pub fn from_mcp(tool: &'a McpTool) -> Self {
        Self {
            name: &tool.exposed_name,
            params: tool
                .params
                .iter()
                .map(|param| ToolTemplateParam {
                    name: param.name.as_str(),
                    kind: param.kind.as_str(),
                    required: param.required,
                })
                .collect(),
        }
    }
}
