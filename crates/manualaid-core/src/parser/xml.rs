//! XML wire-format parser: `<tool><param>value</param></tool>`. Only the
//! defined tool and parameter tags are parsed; other tags are ignored or
//! kept as literal value text, so model output only needs to escape the
//! tool's own tags (a bare `&` is also kept verbatim). A tool call that is
//! unclosed or carries no defined parameter is ignored and scanning resumes
//! right after its start tag, unless the unclosed tool had at least one
//! valid parameter tag — then it is kept as a failed call.
//! XML 线格式解析器：`<tool><param>value</param></tool>`。只解析已定义的
//! 工具与参数标签；其余标签被忽略或作为参数原文保留，模型只需转义工具
//! 自身的标签（裸 `&` 也原样保留）。未闭合或不含任何已定义参数的工具
//! 调用被忽略，扫描从其开始标签之后继续；但若未闭合工具已扫描到至少
//! 一个合法参数标签，则保留为失败调用。CDATA 包裹扫描到文件末尾仍未
//! 闭合时，参数标签视为未正确闭合并写入软警告。

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
                // EOF：未闭合的工具调用若已扫描到至少一个合法参数标签，
                // 保留为失败调用并从其开始标签之后重新扫描，使后续内容
                // 仍能正常解析；否则整体忽略。参数捕获仍在进行时，同时
                // 标记参数未闭合并写入软警告。CDATA 包裹扫描到 EOF 仍未
                // 闭合也统一在此处理，不再单独区分。
                let Some(open) = tool.take() else {
                    break;
                };
                if open.had_params {
                    let mut unclosed_param = false;
                    if let Some(cap) = param.take() {
                        unclosed_param = true;
                        warnings.push(format!(
                            "Unclosed parameter <{}> of tool <{}> discarded (missing </{}>)",
                            cap.name, open.name, cap.name
                        ));
                    }
                    warnings.push(format!(
                        "Unclosed tool <{name}> kept as failed call (missing </{name}>)",
                        name = open.name
                    ));
                    calls.push(finish_call(
                        &open.name,
                        open.params,
                        open.open_offset,
                        unclosed_param,
                        true,
                    ));
                }
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
                // CDATA 只在参数捕获刚开启且值仍为空串时作为包裹处理：此时
                // `<![CDATA[` 必然紧贴参数开始标签的 `>`（中间有任何字符都会
                // 先被追加进值）。其他情况一律不特殊处理：参数内把前缀按字面
                // 文本推进（后续 `</参数>` 仍能正常闭合），空闲/工具内跳过前缀
                // 继续扫描，绝不落入下方的通用标签解析——否则 `scan_tag_end`
                // 会一路扫到下一个 `>`，吞掉中间的标签。
                let is_wrapping =
                    tool.is_some() && param.as_ref().is_some_and(|cap| cap.value.is_empty());
                if is_wrapping && let (Some(open), Some(cap)) = (&tool, &mut param) {
                    cap.is_cdata_wrapped = true;
                    pos = append_cdata(input, p, &open.name, &cap.name, &mut cap.value);
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
            let (name, name_end) = read_tag_name(input, if closing { p + 2 } else { p + 1 });

            // `<` 后紧跟 `>`、`/` 或空白等非法名称起始字符时不构成合法
            // 标签；若继续按标签扫描会贪婪吞掉后续真实闭合标签（如参数
            // 值中的 `p < target` 吞掉 `</content>`）。参数捕获内把 `<` 按
            // 字面文本保留，只推进一个字符；空闲/工具内状态则跳过 `<`。
            if name.is_empty() {
                if let Some(cap) = param.as_mut() {
                    cap.value.push('<');
                }
                pos = p + 1;
                continue;
            }

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
                                is_cdata_wrapped: false,
                            });
                        }
                    } else if closing && name == open.name {
                        // 工具闭合：未收集到任何有效参数的工具调用被忽略。
                        if open.had_params {
                            calls.push(finish_call(
                                &open.name,
                                std::mem::take(&mut open.params),
                                open.open_offset,
                                false,
                                false,
                            ));
                        }
                        tool = None;
                    }
                }
                (Some(open), Some(cap)) => {
                    // 参数内：只有本参数的闭合标签结束捕获；本工具的闭合
                    // 标签抛弃参数（警告）并结束调用；其余一切按原文保留。
                    if closing && name == cap.name {
                        let mut value = std::mem::take(&mut cap.value);
                        if cap.is_cdata_wrapped {
                            // CDATA 包裹：去掉尾随空白（通常是格式化换行）
                            value = value.trim_end().to_string();
                        } else {
                            // 非 CDATA 包裹：只去掉首尾换行符，保留缩进
                            value = value
                                .trim_matches(|c: char| c == '\n' || c == '\r')
                                .to_string();
                        }
                        // 允许空字符串参数（如 new_string 为空表示删除）
                        open.params.insert(cap.name.clone(), Value::String(value));
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
                            true,
                            false,
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
    /// 是否通过 CDATA 包裹捕获（影响空白处理策略）。
    is_cdata_wrapped: bool,
}

/// 完成一个工具调用并入队。
fn finish_call(
    tool_name: &str,
    params: IndexMap<String, Value>,
    offset: usize,
    unclosed_param: bool,
    unclosed_tool: bool,
) -> ParsedToolCall {
    ParsedToolCall {
        tool_name: tool_name.to_string(),
        params,
        format: ToolCallFormat::Xml,
        source_offset: Some(offset),
        unclosed_param,
        unclosed_tool,
    }
}

/// 从 `from` 起下一个 `<` 的位置，不存在时返回 `None`。
pub(super) fn next_angle(input: &str, from: usize) -> Option<usize> {
    input[from..].find('<').map(|rel| from + rel)
}

/// 读取自 `start`（`<` 或 `</` 之后的首个字符）开始的标签名。名称止于
/// 第一个 `>`、`/` 或空白字符；返回名称及紧随其后的位置。
pub(super) fn read_tag_name(input: &str, start: usize) -> (&str, usize) {
    let rest = &input[start..];
    let end = rest
        .find(|c: char| c == '>' || c == '/' || c.is_ascii_whitespace())
        .unwrap_or(rest.len());
    (&rest[..end], start + end)
}

/// 从 `after_name`（名称之后）扫描到标签结束。返回（是否自闭合，`>` 后
/// 的位置）；属性值内的 `/` 与 `>` 不被误判。
pub(super) fn scan_tag_end(input: &str, after_name: usize) -> (bool, usize) {
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
pub(super) fn skip_comment(input: &str, p: usize) -> usize {
    input[p + 4..]
        .find("-->")
        .map(|rel| p + 4 + rel + 3)
        .unwrap_or(input.len())
}

/// 将自 `p`（指向 `<![CDATA[`）开始的 CDATA 内容追加到 `value`。`]]>`
/// 仅当其后的闭合标签是参数或工具的闭合标签时才视为 CDATA 结束，否则
/// 作为内容原文继续扫描（模型可能在内容里写出 `]]>`，如提示词示例）。
/// 返回 CDATA 结束后的偏移（或 `input.len()`）。
pub(super) fn append_cdata(
    input: &str,
    p: usize,
    tool: &str,
    param: &str,
    value: &mut String,
) -> usize {
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

/// `]]>`（位于 `end`）之后是否紧贴参数或工具的闭合标签（`]]>` 与 `</`
/// 之间不允许任何字符，含空白）。
pub(super) fn cdata_close_follows(input: &str, end: usize, tool: &str, param: &str) -> bool {
    let Some(rest) = input[end + 3..].strip_prefix("</") else {
        return false;
    };
    let name = rest
        .split(|c: char| c == '>' || c.is_ascii_whitespace())
        .next()
        .unwrap_or("");
    name == param || name == tool
}

/// 解码一段文本中的实体引用：预定义实体与数字引用还原；裸 `&` 与未知
/// 引用按字面保留（逐引用处理，避免整段因一个裸 `&` 而无法解码）。
pub(super) fn decode_text(text: &str) -> String {
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
pub(super) fn is_entity(name: &str) -> bool {
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
pub(super) fn decode_entity(raw: &str) -> String {
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
pub(super) fn decode_numeric_ref(raw: &str) -> Option<char> {
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
pub(super) fn is_xml_char(c: char) -> bool {
    matches!(
        c as u32,
        0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
    )
}

#[cfg(test)]
mod tests;
