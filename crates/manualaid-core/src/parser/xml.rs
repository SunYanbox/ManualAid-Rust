//! XML wire-format parser: `<tool><param>value</param></tool>`. Only the
//! defined tool and parameter tags are parsed; other tags are ignored or
//! kept as literal value text, so model output only needs to escape the
//! tool's own tags (a bare `&` is also kept verbatim). A tool call that is
//! unclosed or carries no defined parameter is ignored and scanning resumes
//! right after its start tag.
//! XML 线格式解析器：`<tool><param>value</param></tool>`。只解析已定义的
//! 工具与参数标签；其余标签被忽略或作为参数原文保留，模型只需转义工具
//! 自身的标签（裸 `&` 也原样保留）。未闭合或不含任何已定义参数的工具
//! 调用被忽略，扫描从其开始标签之后继续。

use indexmap::IndexMap;
use serde_json::Value;

use super::tool_set::EnabledToolSet;
use super::traits::{ParseError, ParseOutcome, ParsedToolCall, ToolCallFormatParser};
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// Parser for the home-grown XML tool-call format.
/// 自研 XML 工具调用格式的解析器。
pub struct XmlParser;

impl ToolCallFormatParser for XmlParser {
    fn format_name(&self) -> &'static str {
        "xml"
    }

    fn try_parse(&self, input: &str, tools: &EnabledToolSet) -> Result<ParseOutcome, ParseError> {
        let mut calls: Vec<ParsedToolCall> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut pos = 0usize;
        // 当前正在解析的工具元素（若已打开）。
        let mut tool: Option<OpenTool> = None;
        // 当前工具内的参数捕获（若已开始）。
        let mut param: Option<ParamCapture> = None;

        loop {
            let Some(p) = next_angle(input, pos) else {
                // EOF：未闭合的工具调用整体忽略，并从其开始标签之后重新
                // 扫描，使后续内容（如其他工具调用）仍能正常解析。
                let Some(open) = tool.take() else {
                    break;
                };
                param = None;
                pos = open.open_end;
                continue;
            };
            // 标签前的文本：仅参数捕获内需要解码追加，其余状态一律丢弃。
            if tool.is_some()
                && let Some(cap) = param.as_mut()
            {
                cap.value.push_str(&decode_text(&input[pos..p]));
            }

            if input[p..].starts_with("<!--") {
                pos = skip_comment(input, p);
                continue;
            }
            if input[p..].starts_with("<![CDATA[") {
                // CDATA 只在参数捕获刚开始（值仍为空白，即紧贴参数开始
                // 标签）时作为包裹处理；参数值中间出现的 `<![CDATA[` 按
                // 普通文本原文保留。非参数状态（空闲/工具内）直接跳过。
                let wraps_value = param.as_ref().is_none_or(|cap| cap.value.trim().is_empty());
                if wraps_value {
                    if let (Some(open), Some(cap)) = (&tool, &mut param) {
                        pos = append_cdata(input, p, &open.name, &cap.name, &mut cap.value);
                    } else {
                        pos = skip_cdata(input, p);
                    }
                    continue;
                }
            }

            // 孤立 `<`（后无内容）在参数内按原文保留。
            if p + 1 >= input.len() {
                if let Some(cap) = param.as_mut() {
                    cap.value.push('<');
                }
                pos = input.len();
                continue;
            }

            let closing = input.as_bytes()[p + 1] == b'/';
            let (name, name_end) = read_tag_name(input, if closing { p + 2 } else { p + 1 });
            let (self_closing, tag_end) = scan_tag_end(input, name_end);

            match (&mut tool, &mut param) {
                (None, _) => {
                    // 空闲：只有已定义的工具标签会打开调用；其余标签透明，
                    // 其子内容继续被扫描（如 `<tool_calls>` 包裹层）。
                    // 自闭合的工具标签没有参数，视为空调用忽略。
                    if !closing && !self_closing && tools.contains_tool(name) {
                        tool = Some(OpenTool {
                            name: name.to_string(),
                            params: IndexMap::new(),
                            open_offset: p,
                            open_end: tag_end,
                            had_params: false,
                        });
                    }
                }
                (Some(open), None) => {
                    // 工具内：只捕获本工具已定义的参数；本工具的闭合标签
                    // 结束调用；其余标签（含其他工具名）连同内容整体忽略。
                    if !closing && tools.contains_param(&open.name, name) {
                        open.had_params = true;
                        if self_closing {
                            open.params
                                .entry(name.to_string())
                                .or_insert_with(|| Value::String(String::new()));
                        } else {
                            param = Some(ParamCapture {
                                name: name.to_string(),
                                value: String::new(),
                            });
                        }
                    } else if closing && name == open.name {
                        // 工具闭合：未收集到任何有效参数的工具调用被忽略。
                        if open.had_params {
                            calls.push(finish_call(
                                &open.name,
                                std::mem::take(&mut open.params),
                                open.open_offset,
                            ));
                        }
                        tool = None;
                    }
                }
                (Some(open), Some(cap)) => {
                    // 参数内：只有本参数的闭合标签结束捕获；本工具的闭合
                    // 标签抛弃参数（警告）并结束调用；其余一切按原文保留。
                    if closing && name == cap.name {
                        let trimmed = std::mem::take(&mut cap.value).trim().to_string();
                        if !trimmed.is_empty() || !open.params.contains_key(&cap.name) {
                            open.params.insert(cap.name.clone(), Value::String(trimmed));
                        }
                        param = None;
                    } else if closing && name == open.name {
                        // 本工具已收集过参数（进入过参数捕获），调用保留，
                        // 仅被抛弃的未闭合参数写入警告。
                        warnings.push(format!(
                            "Unclosed parameter <{}> of tool <{}> discarded (missing </{}>)",
                            cap.name, open.name, cap.name
                        ));
                        calls.push(finish_call(
                            &open.name,
                            std::mem::take(&mut open.params),
                            open.open_offset,
                        ));
                        tool = None;
                        param = None;
                    } else {
                        cap.value.push_str(&input[p..tag_end]);
                    }
                }
            }
            pos = tag_end;
        }

        Ok(ParseOutcome { calls, warnings })
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
            "  <!-- inside a value, escape only this tool's own tags: <{name}> </{name}> and each <param> </param> (< as &lt;, > as &gt;; a raw & is kept as-is); other tags are kept verbatim; or wrap values in <![CDATA[...]]> -->\n</{name}>"
        ));
        out
    }
}

/// 正在解析的工具元素及其已收集的参数。
struct OpenTool {
    name: String,
    params: IndexMap<String, Value>,
    /// 工具开始标签的字节偏移，用于调用的 `source_offset`。
    open_offset: usize,
    /// 工具开始标签之后的位置；工具未闭合时从这里重新扫描。
    open_end: usize,
    /// 是否收集过有效参数标签（未收集任何参数的工具调用被忽略）。
    had_params: bool,
}

/// 正在捕获的参数内容。
struct ParamCapture {
    name: String,
    value: String,
}

/// 完成一个工具调用并入队。
fn finish_call(tool_name: &str, params: IndexMap<String, Value>, offset: usize) -> ParsedToolCall {
    ParsedToolCall {
        tool_name: tool_name.to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: Some(offset),
    }
}

/// 从 `from` 起下一个 `<` 的位置，不存在时返回 `None`。
fn next_angle(input: &str, from: usize) -> Option<usize> {
    input[from..].find('<').map(|rel| from + rel)
}

/// 读取自 `start`（`<` 或 `</` 之后的首个字符）开始的标签名。名称止于
/// 第一个 `>`、`/` 或空白字符；返回名称及紧随其后的位置。
fn read_tag_name(input: &str, start: usize) -> (&str, usize) {
    let rest = &input[start..];
    let end = rest
        .find(|c: char| c == '>' || c == '/' || c.is_ascii_whitespace())
        .unwrap_or(rest.len());
    (&rest[..end], start + end)
}

/// 从 `after_name`（名称之后）扫描到标签结束。返回（是否自闭合，`>` 后
/// 的位置）；属性值内的 `/` 与 `>` 不被误判。
fn scan_tag_end(input: &str, after_name: usize) -> (bool, usize) {
    let rest = &input[after_name..];
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    let mut self_closing = false;
    while i < rest.len() {
        let c = rest[i..].chars().next().unwrap();
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += c.len_utf8();
        } else {
            match c {
                '"' | '\'' => {
                    quote = Some(c);
                    i += 1;
                }
                '>' => return (self_closing, after_name + i + 1),
                // 仅当 `/` 后（忽略空白）紧跟 `>` 才算自闭合。
                '/' if rest[i + 1..].trim_start().starts_with('>') => {
                    self_closing = true;
                    i += 1;
                }
                _ => i += c.len_utf8(),
            }
        }
    }
    (self_closing, input.len())
}

/// 自 `p`（指向 `<`）开始的注释结束后的位置。
fn skip_comment(input: &str, p: usize) -> usize {
    input[p + 4..]
        .find("-->")
        .map(|rel| p + 4 + rel + 3)
        .unwrap_or(input.len())
}

/// 将自 `p`（指向 `<![CDATA[`）开始的 CDATA 内容追加到 `value`。`]]>`
/// 仅当其后的闭合标签是参数或工具的闭合标签时才视为 CDATA 结束，否则
/// 作为内容原文继续扫描（模型可能在内容里写出 `]]>`，如提示词示例）。
/// 返回 CDATA 结束后的偏移（或 `input.len()`）。
fn append_cdata(input: &str, p: usize, tool: &str, param: &str, value: &mut String) -> usize {
    let mut cursor = p + 9;
    loop {
        let Some(rel) = input[cursor..].find("]]>") else {
            value.push_str(&input[cursor..]);
            return input.len();
        };
        let end = cursor + rel;
        value.push_str(&input[cursor..end]);
        if cdata_close_follows(input, end, tool, param) {
            return end + 3;
        }
        // 假 `]]>`：把结束标记本身也作为原文追加，继续扫描。
        value.push_str("]]>");
        cursor = end + 3;
    }
}

/// `]]>`（位于 `end`）之后（跳过空白）是否紧跟参数或工具的闭合标签。
fn cdata_close_follows(input: &str, end: usize, tool: &str, param: &str) -> bool {
    let after = input[end + 3..].trim_start();
    let Some(rest) = after.strip_prefix("</") else {
        return false;
    };
    let name = rest
        .split(|c: char| c == '>' || c.is_ascii_whitespace())
        .next()
        .unwrap_or("");
    name == param || name == tool
}

/// 自 `p`（指向 `<![CDATA[`）开始的 CDATA 结束后的位置（不解析内容）。
fn skip_cdata(input: &str, p: usize) -> usize {
    input[p + 9..]
        .find("]]>")
        .map(|rel| p + 9 + rel + 3)
        .unwrap_or(input.len())
}

/// 解码一段文本中的实体引用：预定义实体与数字引用还原；裸 `&` 与未知
/// 引用按字面保留（逐引用处理，避免整段因一个裸 `&` 而无法解码）。
fn decode_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let candidate = &rest[amp..];
        let Some(semi) = candidate.find(';') else {
            out.push_str(candidate);
            rest = "";
            break;
        };
        let raw = &candidate[..=semi];
        if is_entity(&candidate[1..semi]) {
            out.push_str(&decode_entity(raw));
            rest = &candidate[semi + 1..];
        } else {
            out.push('&');
            rest = &candidate[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `&` 与 `;` 之间的名称是否为可解码的实体（预定义或数字引用）。
fn is_entity(name: &str) -> bool {
    matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
        || name
            .strip_prefix("#x")
            .is_some_and(|hex| !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()))
        || name
            .strip_prefix('#')
            .is_some_and(|dec| !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit()))
}

/// 解码一个完整实体引用（`&amp;` 等预定义实体或 `&#NN;` / `&#xNN;` 数字
/// 引用）；无法解码的数字引用按字面原样返回。调用方已通过 `is_entity`
/// 预筛，这里只会收到合法的实体名。
fn decode_entity(raw: &str) -> String {
    match raw {
        "&amp;" => "&".to_string(),
        "&lt;" => "<".to_string(),
        "&gt;" => ">".to_string(),
        "&quot;" => "\"".to_string(),
        "&apos;" => "'".to_string(),
        _ => decode_numeric_ref(raw).map_or_else(|| raw.to_string(), |c| c.to_string()),
    }
}

/// 解码数字引用 `&#NN;` / `&#xNN;`；无对应 XML 字符时返回 `None`。
fn decode_numeric_ref(raw: &str) -> Option<char> {
    let inner = &raw[1..raw.len() - 1];
    let codepoint = inner
        .strip_prefix("#x")
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .or_else(|| {
            inner
                .strip_prefix('#')
                .and_then(|dec| dec.parse::<u32>().ok())
        })?;
    let c = char::from_u32(codepoint)?;
    is_xml_char(c).then_some(c)
}

/// XML 1.0 合法字符（拒绝控制字符与代理区）。
fn is_xml_char(c: char) -> bool {
    matches!(
        c as u32,
        0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::EnabledToolSet;

    fn parse(input: &str) -> ParseOutcome {
        XmlParser.try_parse(input, &EnabledToolSet::all()).unwrap()
    }

    #[test]
    fn parses_simple_tool_call() {
        let outcome = parse("<read>\n  <file_path>/test/file.txt</file_path>\n</read>");
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(outcome.calls[0].tool_name, "read");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("file_path")
                .and_then(Value::as_str),
            Some("/test/file.txt")
        );
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn parses_multiple_tool_calls() {
        let outcome = parse(
            "<read><file_path>/a.txt</file_path></read><read><file_path>/b.txt</file_path></read>",
        );
        assert_eq!(outcome.calls.len(), 2);
        assert_eq!(outcome.calls[1].tool_name, "read");
    }

    #[test]
    fn empty_input_yields_no_calls() {
        let outcome = parse("");
        assert!(outcome.calls.is_empty());
    }

    #[test]
    fn self_closing_param_is_empty_string() {
        let outcome = parse("<read><file_path /></read>");
        assert!(outcome.calls[0].params.contains_key("file_path"));
        assert_eq!(
            outcome.calls[0]
                .params
                .get("file_path")
                .and_then(Value::as_str),
            Some("")
        );
    }

    #[test]
    fn self_closing_tool_is_ignored() {
        let outcome = parse("<read/>");
        assert!(outcome.calls.is_empty());
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn uppercase_tags_are_ignored() {
        let outcome = parse("<read><Error>boom</Error><file_path>/a</file_path></read>");
        assert!(!outcome.calls[0].params.contains_key("Error"));
        assert!(outcome.calls[0].params.contains_key("file_path"));
    }

    #[test]
    fn param_value_keeps_other_tool_tags_literal() {
        // 参数原文保留：值中的其他工具标签（即使未闭合）不按结构解析。
        let outcome = parse(
            "<edit><file_path>/f</file_path><old_string>if <read> then</old_string><new_string>ok</new_string></edit>",
        );
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(
            outcome.calls[0]
                .params
                .get("old_string")
                .and_then(Value::as_str),
            Some("if <read> then")
        );
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn ignores_formatting_tags_between_calls() {
        let outcome = parse(
            "<read><file_path>/a</file_path></read><indent>  </indent><edit><file_path>/b</file_path><old_string>x</old_string><new_string>y</new_string></edit>",
        );
        assert_eq!(outcome.calls.len(), 2);
        assert_eq!(outcome.calls[0].tool_name, "read");
        assert_eq!(outcome.calls[1].tool_name, "edit");
    }

    #[test]
    fn parses_through_wrapper_elements() {
        let outcome = parse(
            "<tool_calls><read><file_path>/a</file_path></read><edit><file_path>/b</file_path><old_string>x</old_string><new_string>y</new_string></edit></tool_calls>",
        );
        assert_eq!(outcome.calls.len(), 2);
        assert_eq!(outcome.calls[0].tool_name, "read");
        assert_eq!(outcome.calls[1].tool_name, "edit");
    }

    #[test]
    fn ignores_unknown_tool_elements() {
        let outcome = parse("<nonsense><x>1</x></nonsense>");
        assert!(outcome.calls.is_empty());
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn ignores_unknown_param_tags() {
        let outcome = parse("<read><file_path>/a</file_path><bogus>zzz</bogus></read>");
        assert!(!outcome.calls[0].params.contains_key("bogus"));
    }

    #[test]
    fn ignores_nested_tool_inside_call() {
        let outcome =
            parse("<read><file_path>/a</file_path><edit><new_string>y</new_string></edit></read>");
        assert!(outcome.calls[0].params.contains_key("file_path"));
        assert!(!outcome.calls[0].params.contains_key("new_string"));
    }

    #[test]
    fn unclosed_param_discarded_with_warning() {
        let outcome = parse("<edit><old_string>x</edit>");
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(outcome.calls[0].tool_name, "edit");
        assert!(!outcome.calls[0].params.contains_key("old_string"));
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("<old_string>"));
    }

    #[test]
    fn unclosed_tool_is_ignored() {
        // EOF 时仍未闭合的工具调用整体忽略，不产生错误。
        let outcome = parse("<edit><old_string>x</old_string>");
        assert!(outcome.calls.is_empty());
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn unmatched_close_tag_is_ignored() {
        let outcome = parse("<read><file_path>/a</file_path></read></tool_calls>");
        assert_eq!(outcome.calls.len(), 1);
    }

    #[test]
    fn unclosed_tool_at_eof_is_ignored_without_panicking() {
        // EOF 处工具未闭合（含参数内孤立 `<` 的输入）整体忽略，不 panic。
        assert!(parse("<edit><old_string>x").calls.is_empty());
        assert!(parse("<edit><old_string>a <").calls.is_empty());
    }

    #[test]
    fn tag_attributes_do_not_break_param_capture() {
        // 带属性的参数标签照常捕获；属性值内的 `/` 不误判为自闭合。
        let outcome = parse("<read><file_path x=\"a/b\">v</file_path></read>");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("file_path")
                .and_then(Value::as_str),
            Some("v")
        );
    }

    #[test]
    fn unterminated_tag_at_eof_is_ignored() {
        // 未闭合到 `>` 的标签在 EOF 处结束扫描，工具调用被忽略。
        assert!(parse("<read><file_path").calls.is_empty());
    }

    #[test]
    fn empty_tool_call_is_ignored() {
        assert!(parse("<read></read>").calls.is_empty());
    }

    #[test]
    fn tool_without_params_is_ignored() {
        // 闭合标签之间只有文本或未知标签，没有任何有效参数 → 忽略。
        assert!(parse("<read>some text only</read>").calls.is_empty());
        assert!(parse("<read><foo>x</foo></read>").calls.is_empty());
    }

    #[test]
    fn unclosed_tool_resumes_scanning_after_start_tag() {
        // 用户场景：未知包裹层文本中的 `<read>` 字样被误识别为工具，
        // 未闭合时忽略并从其开始标签之后重新扫描，后续调用不受影响。
        let outcome = parse(
            "<intent>第38行是<read>工具模板中的注释行</intent><shell><command>ls</command></shell>",
        );
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(outcome.calls[0].tool_name, "shell");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("command")
                .and_then(Value::as_str),
            Some("ls")
        );
    }

    #[test]
    fn cdata_in_param_kept_raw() {
        let outcome = parse(
            "<edit><old_string><![CDATA[<edit> & <read>]]></old_string><new_string>x</new_string></edit>",
        );
        assert_eq!(
            outcome.calls[0]
                .params
                .get("old_string")
                .and_then(Value::as_str),
            Some("<edit> & <read>")
        );
    }

    #[test]
    fn cdata_with_embedded_close_tags_keeps_full_content() {
        // 用户场景：CDATA 内容里含 `<![CDATA[...]]>` 提示与 `<content>`/
        // `</content>` 模板示例时，仍完整保留到真正的 `]]></content>`。
        let outcome = parse(
            "<write><file_path>/f</file_path><content><![CDATA[# 提示词\nwrap values in <![CDATA[...]]>\n<write><content>${content}</content></write>\nEND]]></content></write>",
        );
        let content = outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str)
            .unwrap();
        assert!(content.contains("wrap values in <![CDATA[...]]>"));
        assert!(content.contains("<content>${content}</content>"));
        assert!(content.ends_with("END"));
    }

    #[test]
    fn cdata_outside_param_is_skipped() {
        // 非参数状态的 CDATA 整体跳过，不影响后续解析。
        let outcome = parse("<![CDATA[noise]]><read><file_path>/a</file_path></read>");
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(outcome.calls[0].tool_name, "read");
    }

    #[test]
    fn unterminated_cdata_at_eof_is_ignored() {
        // CDATA 包裹到 EOF 仍未闭合：工具未闭合被整体忽略，不 panic。
        let outcome = parse("<read><file_path><![CDATA[x");
        assert!(outcome.calls.is_empty());
    }

    #[test]
    fn cdata_mid_value_is_kept_literal() {
        // 参数值中间出现的 `<![CDATA[` 不是包裹，按普通文本原文保留，
        // 参数照常闭合。
        let outcome = parse(
            "<write><file_path>/f</file_path><content>before <![CDATA[x]]> after</content></write>",
        );
        assert_eq!(outcome.calls.len(), 1);
        assert_eq!(
            outcome.calls[0]
                .params
                .get("content")
                .and_then(Value::as_str),
            Some("before <![CDATA[x]]> after")
        );
    }

    #[test]
    fn cdata_after_leading_whitespace_still_wraps() {
        // 参数开始标签后的换行/缩进不影响 CDATA 包裹识别。
        let outcome =
            parse("<write><file_path>/f</file_path><content>\n  <![CDATA[x]]>\n</content></write>");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("content")
                .and_then(Value::as_str),
            Some("x")
        );
    }

    #[test]
    fn cdata_close_followed_by_tool_close() {
        // CDATA 后直接跟工具闭合标签：CDATA 正常结束，参数被抛弃并警告，
        // 工具正常闭合。
        let outcome = parse("<edit><old_string><![CDATA[x]]></edit>");
        assert_eq!(outcome.calls.len(), 1);
        assert!(!outcome.calls[0].params.contains_key("old_string"));
        assert_eq!(outcome.warnings.len(), 1);
    }

    #[test]
    fn tolerates_dangling_ampersand() {
        let outcome = parse("<read><file_path>/a & b</file_path></read>");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("file_path")
                .and_then(Value::as_str),
            Some("/a & b")
        );
    }

    #[test]
    fn decodes_entity_references_in_values() {
        let outcome =
            parse("<read><file_path>a &lt; b &amp; c &#38; d &#x26; e</file_path></read>");
        assert_eq!(
            outcome.calls[0]
                .params
                .get("file_path")
                .and_then(Value::as_str),
            Some("a < b & c & d & e")
        );
    }

    #[test]
    fn decode_text_predefined_and_numeric() {
        assert_eq!(decode_text("&amp;"), "&");
        assert_eq!(decode_text("&lt;"), "<");
        assert_eq!(decode_text("&gt;"), ">");
        assert_eq!(decode_text("&quot;"), "\"");
        assert_eq!(decode_text("&apos;"), "'");
        assert_eq!(decode_text("&#38;"), "&");
        assert_eq!(decode_text("&#x26;"), "&");
    }

    #[test]
    fn decode_text_keeps_bare_amp_and_unknown_ref() {
        assert_eq!(decode_text("a & b"), "a & b");
        assert_eq!(decode_text("&unknown;"), "&unknown;");
        assert_eq!(decode_text(""), "");
    }

    #[test]
    fn decode_text_numeric_multibyte_and_invalid() {
        // 多字节字符解码；无对应 XML 字符的数字引用按字面保留。
        assert_eq!(decode_text("&#x4E2D;"), "中");
        assert_eq!(decode_text("&#20013;"), "中");
        assert_eq!(decode_text("&#0;"), "&#0;");
        assert_eq!(decode_text("&#xD800;"), "&#xD800;");
        assert_eq!(decode_text("&#x110000;"), "&#x110000;");
    }

    #[test]
    fn template_contains_optional_marker_and_escape_hint() {
        let template = XmlParser.tool_call_template(&ToolKind::Read);
        assert!(template.contains("<read>"));
        assert!(template.contains("<!-- optional -->"));
        assert!(template.contains("CDATA"));
    }
}
