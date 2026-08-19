//! Invoke wire-format parser: `<invoke name="tool"><parameter name="param">value</parameter></invoke>`.
//! Only defined tool names (in `name` attributes) and parameter names are
//! parsed; other tags are ignored or kept as literal value text, so model
//! output only needs to escape the fixed `invoke`/`parameter` tags. CDATA
//! handling follows the same strict rules as the XML parser.
//! invoke 线格式解析器：`<invoke name="tool"><parameter name="param">value</parameter></invoke>`。
//! 只解析已定义的工具名（`name` 属性中）与参数名；其余标签被忽略或作为
//! 参数原文保留，模型只需转义固定的 `invoke`/`parameter` 标签。CDATA
//! 处理与 XML 解析器遵循相同的严格规则。

use indexmap::IndexMap;
use serde_json::Value;

use super::tool_set::EnabledToolSet;
use super::traits::{ParseError, ParseOutcome, ParsedToolCall, ToolCallFormatParser};
use crate::tools::ToolCallFormat;
use crate::tools::ToolKind;

/// The fixed tag name for tool elements in the invoke format.
/// invoke 格式中工具元素的固定标签名。
const TOOL_TAG: &str = "invoke";
/// The fixed tag name for parameter elements in the invoke format.
/// invoke 格式中参数元素的固定标签名。
const PARAM_TAG: &str = "parameter";

/// Parser for the Anthropic-style invoke tool-call format.
/// Anthropic 风格 invoke 工具调用格式的解析器。
pub struct InvokeParser;

impl ToolCallFormatParser for InvokeParser {
    fn format_name(&self) -> &'static str {
        "invoke"
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
            let Some(p) = super::xml::next_angle(input, pos) else {
                // EOF：未闭合的工具调用整体忽略，并从其开始标签之后重新
                // 扫描，使后续内容仍能正常解析。CDATA 包裹扫描到 EOF
                // 仍未闭合时，参数标签视为未正确闭合，写入软警告。
                let Some(open) = tool.take() else {
                    break;
                };
                if let Some(cap) = param.take()
                    && cap.is_cdata_wrapped
                {
                    warnings.push(format!(
                        "Unclosed parameter <{PARAM_TAG} name=\"{}\"> of tool <{TOOL_TAG} name=\"{}\"> discarded (missing </{PARAM_TAG}>)",
                        cap.name, open.name
                    ));
                }
                pos = open.open_end;
                continue;
            };
            // 标签前的文本：仅参数捕获内需要解码追加，其余状态一律丢弃。
            if tool.is_some()
                && let Some(cap) = param.as_mut()
            {
                cap.value.push_str(&super::xml::decode_text(&input[pos..p]));
            }

            if input[p..].starts_with("<!--") {
                pos = super::xml::skip_comment(input, p);
                continue;
            }
            if input[p..].starts_with("<![CDATA[") {
                // CDATA 只在参数捕获刚开启且值仍为空串时作为包裹处理。
                let is_wrapping =
                    tool.is_some() && param.as_ref().is_some_and(|cap| cap.value.is_empty());
                if is_wrapping && let (Some(open), Some(cap)) = (&tool, &mut param) {
                    cap.is_cdata_wrapped = true;
                    pos = super::xml::append_cdata(input, p, TOOL_TAG, PARAM_TAG, &mut cap.value);
                    let _ = &open.name; // keep open borrowed for clarity
                    continue;
                }
                if let Some(cap) = param.as_mut() {
                    cap.value.push_str(&input[p..p + 9]);
                }
                pos = p + 9;
                continue;
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
            let (tag_name, name_end) =
                super::xml::read_tag_name(input, if closing { p + 2 } else { p + 1 });

            // 非法名称起始字符：参数内按字面保留 `<`，其余状态跳过。
            if tag_name.is_empty() {
                if let Some(cap) = param.as_mut() {
                    cap.value.push('<');
                }
                pos = p + 1;
                continue;
            }

            let (self_closing, tag_end) = super::xml::scan_tag_end(input, name_end);

            match (&mut tool, &mut param) {
                (None, _) => {
                    // 空闲：只有 `invoke` 标签会打开调用，且需带有效 `name`
                    // 属性且对应已定义工具。自闭合的 invoke 忽略。其余标签
                    // 透明，其子内容继续被扫描。
                    if !closing
                        && !self_closing
                        && tag_name == TOOL_TAG
                        && let Some(tool_name) = extract_name_attr(input, name_end, tag_end)
                        && tools.contains_tool(&tool_name)
                    {
                        tool = Some(OpenTool {
                            name: tool_name,
                            params: IndexMap::new(),
                            open_offset: p,
                            open_end: tag_end,
                            had_params: false,
                        });
                    }
                }
                (Some(open), None) => {
                    // 工具内：`parameter` 标签开始参数捕获（需带有效 `name`
                    // 属性且为当前工具的已定义参数）；`/invoke` 闭合标签结束
                    // 调用；新的 `<invoke>` 开始标签关闭当前调用并开启新调用
                    // （invoke 格式闭合标签固定为 `</invoke>`，不像 XML 每个
                    // 工具有不同闭合标签，因此需要显式处理嵌套 invoke 以避免
                    // 未闭合调用吞掉后续调用的闭合标签）；其余标签忽略。
                    if !closing && tag_name == TOOL_TAG {
                        if open.had_params {
                            calls.push(finish_call(
                                &open.name,
                                std::mem::take(&mut open.params),
                                open.open_offset,
                            ));
                        }
                        if !self_closing
                            && let Some(tool_name) = extract_name_attr(input, name_end, tag_end)
                            && tools.contains_tool(&tool_name)
                        {
                            tool = Some(OpenTool {
                                name: tool_name,
                                params: IndexMap::new(),
                                open_offset: p,
                                open_end: tag_end,
                                had_params: false,
                            });
                        } else {
                            tool = None;
                        }
                    } else if !closing
                        && tag_name == PARAM_TAG
                        && let Some(param_name) = extract_name_attr(input, name_end, tag_end)
                        && tools.contains_param(&open.name, &param_name)
                    {
                        open.had_params = true;
                        if self_closing {
                            open.params
                                .entry(param_name)
                                .or_insert_with(|| Value::String(String::new()));
                        } else {
                            param = Some(ParamCapture {
                                name: param_name,
                                value: String::new(),
                                is_cdata_wrapped: false,
                            });
                        }
                    } else if closing && tag_name == TOOL_TAG {
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
                    // 参数内：只有 `/parameter` 闭合标签结束捕获；`/invoke`
                    // 闭合标签抛弃参数（警告）并结束调用；其余一切按原文保留。
                    if closing && tag_name == PARAM_TAG {
                        let mut value = std::mem::take(&mut cap.value);
                        if cap.is_cdata_wrapped {
                            value = value.trim_end().to_string();
                        } else {
                            value = value
                                .trim_matches(|c: char| c == '\n' || c == '\r')
                                .to_string();
                        }
                        open.params.insert(cap.name.clone(), Value::String(value));
                        param = None;
                    } else if closing && tag_name == TOOL_TAG {
                        warnings.push(format!(
                            "Unclosed parameter <{PARAM_TAG} name=\"{}\"> of tool <{TOOL_TAG} name=\"{}\"> discarded (missing </{PARAM_TAG}>)",
                            cap.name, open.name
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
        let mut out = format!("<{TOOL_TAG} name=\"{name}\">\n");
        for param in &params {
            let comment = if param.required {
                ""
            } else {
                "<!-- optional -->"
            };
            out.push_str(&format!(
                "<{PARAM_TAG} name=\"{tag}\">${{{tag}}}{comment}</{PARAM_TAG}>\n",
                tag = param.name,
            ));
        }
        out.push_str(&format!(
            "  <!-- inside a value, escape only the fixed tags: <{TOOL_TAG}> </{TOOL_TAG}> and <{PARAM_TAG}> </{PARAM_TAG}> (< as &lt;, > as &gt;; a raw & is kept as-is); other tags are kept verbatim; or wrap values in <![CDATA[...]]> -->\n</{TOOL_TAG}>"
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
    /// 是否通过 CDATA 包裹捕获（影响空白处理策略）。
    is_cdata_wrapped: bool,
}

/// 完成一个工具调用并入队。
fn finish_call(tool_name: &str, params: IndexMap<String, Value>, offset: usize) -> ParsedToolCall {
    ParsedToolCall {
        tool_name: tool_name.to_string(),
        params,
        format: ToolCallFormat::Invoke,
        source_offset: Some(offset),
    }
}

/// 从标签属性区域中提取 `name="..."` 或 `name='...'` 的值。`start` 是标签名
/// 后的位置，`tag_end` 是 `>` 之后的位置。找不到 `name` 属性时返回 `None`。
fn extract_name_attr(input: &str, start: usize, tag_end: usize) -> Option<String> {
    // tag_end 指向 `>` 之后，属性区域是 [start, tag_end-1)。
    let attr_end = if tag_end > 0 { tag_end - 1 } else { tag_end };
    let attrs = &input[start..attr_end];
    let mut i = 0usize;
    let bytes = attrs.as_bytes();
    while i < attrs.len() {
        // 跳过空白
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        // 读取属性名
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && bytes[i] != b'='
            && bytes[i] != b'/'
        {
            i += 1;
        }
        let attr_name = &attrs[name_start..i];
        // 跳过空白
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        // 需要 `=`
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1; // 跳过 `=`
        // 跳过空白
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            // 无引号的属性值不支持
            continue;
        }
        i += 1; // 跳过引号
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let value = &attrs[value_start..i];
        if i < bytes.len() {
            i += 1; // 跳过闭合引号
        }
        if attr_name == "name" {
            return Some(super::xml::decode_text(value));
        }
    }
    None
}

#[cfg(test)]
mod tests;
