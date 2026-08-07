//! JSON code-block wire-format parser: Anthropic-style fenced
//! ` ```json … ``` ` blocks with `tool_use` / `params` objects, plus a
//! top-level array form and inline JSON detection.
//! JSON 代码块线格式解析器：Anthropic 风格围栏 ` ```json … ``` ` 块
//! （`tool_use` / `params` 对象），另支持顶层数组形式与内联 JSON 检测。

use indexmap::IndexMap;
use serde_json::Value;

use super::traits::{ParseError, ParsedToolCall, ToolCallFormatParser};
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// Parser for the JSON-fenced-code-block tool-call format.
/// JSON 围栏代码块工具调用格式的解析器。
pub struct JsonCodeblockParser;

impl ToolCallFormatParser for JsonCodeblockParser {
    fn format_name(&self) -> &'static str {
        "json-codeblock"
    }

    fn try_parse(&self, input: &str) -> Result<Vec<ParsedToolCall>, ParseError> {
        let blocks = extract_json_blocks(input);
        let mut calls = Vec::new();
        for (block, offset) in &blocks {
            calls.extend(parse_json_tool_call(block, *offset)?);
        }
        Ok(calls)
    }

    fn tool_call_template(&self, tool: &ToolKind) -> String {
        let name = tool.name();
        let params = tool.parameters();
        let mut out = format!("{{\n  \"tool_use\": \"{name}\",\n  \"params\": {{");
        for param in &params {
            let comment = if param.required { "" } else { " // optional" };
            let value = match param.kind {
                "integer" | "number" => "0",
                "boolean" => "true",
                _ => "\"<value>\"",
            };
            out.push_str(&format!("\n    \"{}\": {}{}", param.name, value, comment));
        }
        out.push_str("\n  }\n}");
        out.push_str(
            "\nNote: all string values must be valid JSON strings: escape `\"` as `\\\"`, `\\` as `\\\\`, newline as `\\n`.",
        );
        out
    }
}

/// Extract fenced JSON code blocks (` ```json … ``` `) from `input`.
/// When no fence is found, inline JSON objects/arrays containing a
/// `"tool_use"` field are detected.
/// 从 `input` 中提取围栏 JSON 代码块（` ```json … ``` `）。
/// 未找到围栏时，检测包含 `"tool_use"` 字段的内联 JSON 对象/数组。
fn extract_json_blocks(input: &str) -> Vec<(String, usize)> {
    let mut blocks = Vec::new();
    let mut remaining = input;

    while !remaining.is_empty() {
        let consumed = input.len() - remaining.len();

        let Some((after_fence, fence_len)) = find_fence(remaining) else {
            break;
        };
        let offset_adj = consumed + fence_len;

        if let Some(end) = after_fence.find("```") {
            let content = &after_fence[..end].trim_end();
            blocks.push((content.to_string(), offset_adj));
            remaining = &after_fence[end + 3..];
        } else {
            let content = after_fence.trim_end();
            blocks.push((content.to_string(), offset_adj));
            break;
        }
    }

    if blocks.is_empty() {
        let trimmed = input.trim();
        let is_tool_json = (trimmed.starts_with('{') || trimmed.starts_with('['))
            && trimmed.contains("\"tool_use\"");
        if is_tool_json {
            blocks.push((trimmed.to_string(), 0));
        }
    }

    blocks
}

/// Locate the next JSON fence: ` ```json\n `, ` ```json\r\n `, or a bare
/// ` ```\n ` whose content starts with `{` or `[`. Returns the text after
/// the fence plus the length of the fence prefix itself.
/// 定位下一个 JSON 围栏：` ```json\n `、` ```json\r\n `，或内容以 `{` /
/// `[` 开头的裸 ` ```\n `。返回围栏后的文本及该文本的绝对字符偏移。
fn find_fence(remaining: &str) -> Option<(&str, usize)> {
    for (fence, content) in [("```json\n", true), ("```json\r\n", true), ("```\n", false)] {
        if let Some(pos) = remaining.find(fence) {
            let after = &remaining[pos + fence.len()..];
            if !content && !looks_like_json(after) {
                continue;
            }
            return Some((after, pos + fence.len()));
        }
    }
    None
}

/// Whether the text after a bare fence looks like JSON.
/// 裸围栏后的文本是否看起来像 JSON。
fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

/// Parse a single JSON value into one or more tool calls.
/// 将单个 JSON 值解析为一个或多个工具调用。
fn parse_json_tool_call(json_text: &str, offset: usize) -> Result<Vec<ParsedToolCall>, ParseError> {
    let value: Value = serde_json::from_str(json_text).map_err(|e| {
        ParseError::new(format!("JSON parse error: {e}"))
            .with_offset(offset)
            .with_format(ToolCallFormat::JsonCodeblock)
            .with_cause(e.to_string())
    })?;

    match value {
        Value::Array(items) => {
            let mut calls = Vec::new();
            for item in items {
                calls.push(tool_call_from_object(item, offset)?);
            }
            Ok(calls)
        }
        Value::Object(_) => Ok(vec![tool_call_from_object(value, offset)?]),
        _ => Err(
            ParseError::new("Expected a JSON object or array of tool calls")
                .with_offset(offset)
                .with_format(ToolCallFormat::JsonCodeblock),
        ),
    }
}

/// Convert a JSON object into a `ParsedToolCall`.
/// 将 JSON 对象转换为 `ParsedToolCall`。
fn tool_call_from_object(obj: Value, offset: usize) -> Result<ParsedToolCall, ParseError> {
    let map = obj.as_object().ok_or_else(|| {
        ParseError::new(format!("Tool call must be a JSON object, got {obj}"))
            .with_offset(offset)
            .with_format(ToolCallFormat::JsonCodeblock)
    })?;

    let tool_name = map
        .get("tool_use")
        .or_else(|| map.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ParseError::new("Missing \"tool_use\" field in tool call object")
                .with_offset(offset)
                .with_format(ToolCallFormat::JsonCodeblock)
        })?;

    let params = if let Some(params_value) = map.get("params") {
        match params_value {
            Value::Object(pm) => pm
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<IndexMap<_, _>>(),
            other => {
                let mut params = IndexMap::new();
                params.insert("params".to_string(), other.clone());
                params
            }
        }
    } else {
        map.iter()
            .filter(|(key, _)| *key != "tool_use" && *key != "name")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    };

    Ok(ParsedToolCall {
        tool_name,
        params,
        format: ToolCallFormat::JsonCodeblock,
        source_offset: Some(offset),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_name_is_json_codeblock() {
        assert_eq!(JsonCodeblockParser.format_name(), "json-codeblock");
    }

    #[test]
    fn extracts_fenced_block() {
        let blocks = extract_json_blocks("prefix\n```json\n{\"key\": \"val\"}\n```\nsuffix");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
    }

    #[test]
    fn extracts_crlf_fence() {
        let blocks = extract_json_blocks("prefix\r\n```json\r\n{\"key\": \"val\"}\r\n```\r\n");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
    }

    #[test]
    fn bare_fence_skips_non_json() {
        let blocks = extract_json_blocks("```\nsome text\n```\n```json\n{\"a\": 1}\n```");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "{\"a\": 1}");
    }

    #[test]
    fn inline_json_with_tool_use_is_detected() {
        let blocks = extract_json_blocks("{\"tool_use\": \"read\", \"params\": {}}");
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn parses_params_fallback_keys() {
        let calls = JsonCodeblockParser
            .try_parse("```json\n{\"tool_use\": \"read\", \"file_path\": \"/a.txt\"}\n```")
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "read");
        assert_eq!(
            calls[0].params.get("file_path").and_then(Value::as_str),
            Some("/a.txt")
        );
    }

    #[test]
    fn parses_array_of_calls() {
        let calls = JsonCodeblockParser
            .try_parse(
                "```json\n[{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}, {\"tool_use\": \"edit\", \"params\": {\"file_path\": \"/b\"}}]\n```",
            )
            .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].tool_name, "edit");
    }

    #[test]
    fn missing_tool_use_is_an_error() {
        let result = JsonCodeblockParser.try_parse("```json\n{\"params\": {\"x\": \"1\"}}\n```");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("tool_use"));
    }

    #[test]
    fn non_object_value_is_an_error() {
        let result = JsonCodeblockParser.try_parse("```json\n\"just a string\"\n```");
        assert!(result.is_err());
    }

    #[test]
    fn empty_input_yields_no_calls() {
        let calls = JsonCodeblockParser.try_parse("").unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn template_marks_optional_params() {
        let template = JsonCodeblockParser.tool_call_template(&ToolKind::Read);
        assert!(template.contains("\"tool_use\": \"read\""));
        assert!(template.contains("// optional"));
        assert!(template.contains("escape"));
    }
}
