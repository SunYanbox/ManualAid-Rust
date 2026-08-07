//! XML wire-format parser: `<tool><param>value</param></tool>` with a
//! lenient mode where a bare `&` is kept verbatim, so model output only
//! needs to escape `<` and `>`.
//! XML 线格式解析器：`<tool><param>value</param></tool>`；宽容模式下裸
//! `&`（未形成合法引用）原样保留，模型只需转义 `<` 与 `>`。

use indexmap::IndexMap;
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;

use super::traits::{ParseError, ParsedToolCall, ToolCallFormatParser};
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// Parser for the home-grown XML tool-call format.
/// 自研 XML 工具调用格式的解析器。
pub struct XmlParser;

impl ToolCallFormatParser for XmlParser {
    fn format_name(&self) -> &'static str {
        "xml"
    }

    fn try_parse(&self, input: &str) -> Result<Vec<ParsedToolCall>, ParseError> {
        let mut reader = Reader::from_str(input);
        // 宽容模式：裸 `&` 原样保留为文本。
        reader.config_mut().allow_dangling_amp = true;

        let mut calls = Vec::new();
        let mut buf = Vec::new();

        let mut current_tool: Option<String> = None;
        let mut current_params: IndexMap<String, Value> = IndexMap::new();
        let mut current_depth: usize = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if current_tool.is_none() {
                        current_tool = Some(tag_name);
                        current_params = IndexMap::new();
                        current_depth = 1;
                    } else {
                        current_depth += 1;
                    }
                }

                Ok(Event::Empty(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    // Self-closing tags become empty-string values; tags that
                    // start with an uppercase letter (control/metadata) are
                    // never treated as parameters.
                    // 自闭合标签作为空字符串值；大写字母开头的标签
                    // （控制/元数据）永远不视为参数。
                    if current_tool.is_some() && !starts_uppercase(&tag_name) {
                        current_params
                            .entry(tag_name)
                            .or_insert(Value::String(String::new()));
                    }
                }

                Ok(Event::Text(e)) => {
                    if current_tool.is_some() {
                        let text = String::from_utf8_lossy(e.as_ref());
                        insert_text_value(&mut current_params, current_depth, &text);
                    }
                }

                Ok(Event::CData(e)) => {
                    // CDATA does not need XML unescaping.
                    // CDATA 不需要 XML 反转义。
                    if current_tool.is_some() {
                        let content = String::from_utf8_lossy(e.as_ref());
                        insert_text_value(&mut current_params, current_depth, &content);
                    }
                }

                Ok(Event::GeneralRef(e)) => {
                    // Entity references (`&amp;`, `&#38;`, ...) are decoded
                    // and appended as part of the text.
                    // 实体引用（`&amp;`、`&#38;` 等）解码后作为文本的一部分拼接。
                    if current_tool.is_some() {
                        let decoded = decode_ref(&String::from_utf8_lossy(e.as_ref()));
                        if !decoded.is_empty() {
                            insert_text_value(&mut current_params, current_depth, &decoded);
                        }
                    }
                }

                Ok(Event::End(e)) => {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                    if current_depth == 1 && current_tool.as_deref() == Some(&tag_name) {
                        if let Some(tool) = current_tool.take() {
                            current_params.shift_remove("__text_buf_1__");
                            calls.push(ParsedToolCall {
                                tool_name: tool,
                                params: std::mem::take(&mut current_params),
                                format: ToolCallFormat::Xml,
                                source_offset: None,
                            });
                        }
                        current_params = IndexMap::new();
                        current_depth = 0;
                    } else if current_depth > 1 {
                        // Closing a parameter element: trim the accumulated
                        // text and store it under the tag name.
                        // 关闭参数元素：裁剪累积文本并按标签名存储。
                        let buf_key = format!("__text_buf_{current_depth}__");
                        let value = current_params
                            .shift_remove(&buf_key)
                            .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
                            .unwrap_or_default();

                        if !starts_uppercase(&tag_name)
                            && (!value.is_empty() || !current_params.contains_key(&tag_name))
                        {
                            current_params.insert(tag_name, Value::String(value));
                        }
                        current_depth -= 1;
                    }
                }

                Ok(Event::Eof) => break,

                Err(e) => {
                    return Err(ParseError::new(format!("XML parse error: {e}"))
                        .with_offset(reader.buffer_position() as usize)
                        .with_format(ToolCallFormat::Xml));
                }

                _ => {}
            }

            buf.clear();
        }

        Ok(calls)
    }

    fn tool_call_template(&self, tool: &ToolKind) -> String {
        let name = tool.name();
        let params = tool.parameters();
        let mut out = format!("<{name}>\n");
        for param in &params {
            let comment = if param.required {
                ""
            } else {
                "<!-- optional -->"
            };
            out.push_str(&format!(
                "  <{tag}>${{{tag}}}{comment}</{tag}>\n",
                tag = param.name,
            ));
        }
        out.push_str(&format!(
            "  <!-- escape XML special chars in values: < as &lt;, > as &gt; (a raw & is also kept as-is); or wrap values in <![CDATA[...]]> -->\n</{name}>"
        ));
        out
    }
}

/// Whether a tag name starts with an ASCII uppercase letter.
/// 标签名是否以 ASCII 大写字母开头。
fn starts_uppercase(tag: &str) -> bool {
    tag.starts_with(|c: char| c.is_ascii_uppercase())
}

/// Decode the name part of a `&...;` reference (without `&` and `;`).
/// Predefined entities and numeric references are restored; unknown
/// references are returned verbatim as `&name;`.
/// 将 `&...;` 引用的名称部分（不含 `&` 与 `;`）解码为实际字符。
/// 预定义实体与数字字符引用会被还原；无法识别的实体按字面量 `&name;`
/// 原样返回。
fn decode_ref(name: &str) -> String {
    let raw = format!("&{name};");
    quick_xml::escape::unescape(&raw)
        .map(|s| s.into_owned())
        .unwrap_or(raw)
}

/// Insert or append text into the depth-isolated text buffer, so outer
/// element text never leaks into inner element values.
/// 将文本插入或追加到深度隔离的文本缓冲区中，使外层元素的文本不会
/// 泄漏到内层元素的值中。
fn insert_text_value(params: &mut IndexMap<String, Value>, depth: usize, value: &str) {
    let key = format!("__text_buf_{depth}__");
    params
        .entry(key)
        .and_modify(|v| {
            if let Value::String(s) = v {
                s.push_str(value);
            }
        })
        .or_insert_with(|| Value::String(value.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_tool_call() {
        let parser = XmlParser;
        let calls = parser
            .try_parse("<read>\n  <file_path>/test/file.txt</file_path>\n</read>")
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "read");
        assert_eq!(
            calls[0].params.get("file_path").and_then(Value::as_str),
            Some("/test/file.txt")
        );
    }

    #[test]
    fn parses_multiple_tool_calls() {
        let parser = XmlParser;
        let calls = parser
            .try_parse("<read><file_path>/a.txt</file_path></read><read><file_path>/b.txt</file_path></read>")
            .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].tool_name, "read");
    }

    #[test]
    fn empty_input_yields_no_calls() {
        let calls = XmlParser.try_parse("").unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn self_closing_tag_is_empty_string() {
        let calls = XmlParser.try_parse("<tool><flag /></tool>").unwrap();
        assert!(calls[0].params.contains_key("flag"));
    }

    #[test]
    fn uppercase_tags_are_ignored() {
        let calls = XmlParser
            .try_parse("<read><Error>boom</Error><file_path>/a</file_path></read>")
            .unwrap();
        assert!(!calls[0].params.contains_key("Error"));
    }

    #[test]
    fn decode_ref_predefined_and_numeric() {
        assert_eq!(decode_ref("amp"), "&");
        assert_eq!(decode_ref("lt"), "<");
        assert_eq!(decode_ref("gt"), ">");
        assert_eq!(decode_ref("quot"), "\"");
        assert_eq!(decode_ref("apos"), "'");
        assert_eq!(decode_ref("#38"), "&");
        assert_eq!(decode_ref("#x26"), "&");
    }

    #[test]
    fn decode_ref_unknown_passes_through() {
        assert_eq!(decode_ref("unknown"), "&unknown;");
        assert_eq!(decode_ref(""), "&;");
    }

    #[test]
    fn template_contains_optional_marker_and_escape_hint() {
        let template = XmlParser.tool_call_template(&ToolKind::Read);
        assert!(template.contains("<read>"));
        assert!(template.contains("<!-- optional -->"));
        assert!(template.contains("CDATA"));
    }
}
