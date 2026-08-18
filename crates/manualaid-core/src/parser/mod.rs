//! Tool-call parsing: raw text input is converted into structured
//! [`ParsedToolCall`]s by one of the registered format parsers.
//! 工具调用解析：由已注册的格式解析器之一把原始文本输入转换为结构化
//! [`ParsedToolCall`]。
//!
//! # Description
//! Two wire formats are built in: the home-grown XML format and the
//! Anthropic-style JSON code block. [`FormatRegistry`] selects the active
//! parser(s) through [`RegistryMode`] (auto-detect or fixed).
//! # 描述
//! 内置两种线格式：自研 XML 格式与 Anthropic 风格 JSON 代码块。
//! [`FormatRegistry`] 通过 [`RegistryMode`]（自动检测或固定）选择生效的
//! 解析器。

pub mod invoke;
pub mod json_codeblock;
pub mod registry;
pub mod tool_set;
pub mod traits;
pub mod xml;

pub use registry::{FormatRegistry, RegistryMode};
pub use tool_set::EnabledToolSet;
pub use traits::*;
