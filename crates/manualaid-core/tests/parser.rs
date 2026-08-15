//! Integration tests for the parser layer: XML and JSON-codeblock parsing,
//! template rendering, the enabled-tool filtering and the format registry
//! modes.
//! 解析器层集成测试：XML 与 JSON-codeblock 解析、模板渲染、可用工具过滤
//! 与格式注册表模式。

use manualaid_core::parser::{
    EnabledToolSet, FormatRegistry, ParseOutcome, RegistryMode, ToolCallFormatParser,
};
use manualaid_core::tools::{ToolCallFormat, ToolKind};

fn xml(input: &str) -> ParseOutcome {
    manualaid_core::parser::xml::XmlParser
        .try_parse(input, &EnabledToolSet::all())
        .unwrap()
}

fn json(input: &str) -> ParseOutcome {
    manualaid_core::parser::json_codeblock::JsonCodeblockParser
        .try_parse(input, &EnabledToolSet::all())
        .unwrap()
}

#[test]
fn xml_parses_and_renders_template() {
    let parser = manualaid_core::parser::xml::XmlParser;
    let outcome = parser
        .try_parse(
            "<read><file_path>/tmp/a.txt</file_path></read>",
            &EnabledToolSet::all(),
        )
        .unwrap();
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(outcome.calls[0].format, ToolCallFormat::Xml);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("/tmp/a.txt")
    );

    let template = parser.tool_call_template(&ToolKind::Edit);
    assert!(template.contains("<edit>"));
    assert!(template.contains("<file_path>"));
}

#[test]
fn xml_keeps_parameter_order() {
    let parser = manualaid_core::parser::xml::XmlParser;
    let outcome = parser
        .try_parse("<edit><new_string>B</new_string><old_string>A</old_string><file_path>/f</file_path></edit>", &EnabledToolSet::all())
        .unwrap();
    let keys: Vec<&str> = outcome.calls[0].params.keys().map(String::as_str).collect();
    assert_eq!(keys, ["new_string", "old_string", "file_path"]);
}

#[test]
fn xml_tolerates_dangling_ampersand() {
    let outcome = xml("<write><file_path>/f</file_path><content>a & b</content></write>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(|v| v.as_str()),
        Some("a & b")
    );
}

#[test]
fn xml_ignores_formatting_tags_between_calls() {
    let outcome = xml(
        "<read><file_path>/a</file_path></read><indent>  </indent><edit><file_path>/b</file_path><old_string>x</old_string><new_string>y</new_string></edit>",
    );
    assert_eq!(outcome.calls.len(), 2);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(outcome.calls[1].tool_name, "edit");
}

#[test]
fn xml_parses_through_wrapper_elements() {
    let outcome = xml(
        "<tool_calls><read><file_path>/a</file_path></read><edit><file_path>/b</file_path><old_string>x</old_string><new_string>y</new_string></edit></tool_calls>",
    );
    assert_eq!(outcome.calls.len(), 2);
    assert_eq!(outcome.calls[1].tool_name, "edit");
}

#[test]
fn xml_param_value_keeps_other_tool_tags_literal() {
    // 参数原文保留：值中的其他工具标签（即使未闭合）不按结构解析。
    let outcome = xml(
        "<edit><file_path>/f</file_path><old_string>if <read> then</old_string><new_string>ok</new_string></edit>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("if <read> then")
    );
    assert!(outcome.warnings.is_empty());
}

#[test]
fn xml_ignores_unknown_tool_elements() {
    let outcome = xml("<nonsense><x>1</x></nonsense>");
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn xml_ignores_unknown_param_tags() {
    let outcome = xml("<read><file_path>/a</file_path><bogus>zzz</bogus></read>");
    assert!(!outcome.calls[0].params.contains_key("bogus"));
}

#[test]
fn xml_ignores_nested_tool_inside_call() {
    let outcome =
        xml("<read><file_path>/a</file_path><edit><new_string>y</new_string></edit></read>");
    assert!(outcome.calls[0].params.contains_key("file_path"));
    assert!(!outcome.calls[0].params.contains_key("new_string"));
}

#[test]
fn xml_unclosed_param_discarded_with_warning() {
    let outcome = xml("<edit><old_string>x</edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "edit");
    assert!(!outcome.calls[0].params.contains_key("old_string"));
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<old_string>"));
}

#[test]
fn xml_unclosed_tool_is_ignored() {
    // EOF 时仍未闭合的工具调用整体忽略，不产生错误。
    let outcome = xml("<edit><old_string>x</old_string>");
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn xml_tool_tag_in_unknown_wrapper_does_not_break_later_calls() {
    // 用户场景：未知包裹层（如 `<intent>`）的文本里夹带已定义工具名
    // （如 `<read>`），该工具未闭合时被忽略，扫描从其开始标签之后
    // 继续，后续的真实调用不受影响。
    let outcome = xml(
        "<intent>第38行是<read>工具模板中的注释行</intent>\n<shell><command>echo hi</command></shell>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "shell");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("command")
            .and_then(|v| v.as_str()),
        Some("echo hi")
    );
}

#[test]
fn xml_unmatched_close_tag_is_ignored() {
    let outcome = xml("<read><file_path>/a</file_path></read></tool_calls>");
    assert_eq!(outcome.calls.len(), 1);
}

#[test]
fn xml_self_closing_param_is_empty_string() {
    let outcome = xml("<read><file_path /></read>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("")
    );
}

#[test]
fn xml_self_closing_tool_is_ignored() {
    let outcome = xml("<read/>");
    assert!(outcome.calls.is_empty());
}

#[test]
fn json_codeblock_parses_object_and_array() {
    let outcome =
        json("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(outcome.calls[0].format, ToolCallFormat::JsonCodeblock);

    let outcome =
        json("```json\n[{\"tool_use\": \"write\", \"params\": {\"file_path\": \"/a\"}}]\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "write");
}

#[test]
fn json_codeblock_parses_inline_json() {
    let outcome = json("{\"tool_use\": \"shell\", \"params\": {\"command\": \"ls\"}}");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "shell");
}

#[test]
fn json_codeblock_skips_unknown_tool_and_keeps_valid() {
    let outcome = json(
        "```json\n[{\"tool_use\": \"bogus\", \"params\": {}}, {\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}]\n```",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn json_codeblock_filters_unknown_param_keys() {
    let outcome = json(
        "```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\", \"reason\": \"because\"}}\n```",
    );
    assert!(outcome.calls[0].params.contains_key("file_path"));
    assert!(!outcome.calls[0].params.contains_key("reason"));
}

#[test]
fn json_codeblock_name_fallback_is_validated() {
    let outcome = json("```json\n{\"name\": \"read\", \"params\": {\"file_path\": \"/a\"}}\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");

    let outcome = json("```json\n{\"name\": \"bogus\", \"params\": {}}\n```");
    assert!(outcome.calls.is_empty());
}

#[test]
fn malformed_json_returns_structured_error() {
    let parser = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    let error = parser
        .try_parse("```json\nnot json\n```", &EnabledToolSet::all())
        .unwrap_err();
    assert!(error.message.contains("JSON parse error"));
    assert_eq!(error.format, Some(ToolCallFormat::JsonCodeblock));
    assert!(error.to_string().contains("offset"));
}

#[test]
fn json_codeblock_non_object_values_yield_no_calls() {
    for input in ["42", "\"a string\"", "true", "null"] {
        let outcome = json(&format!("```json\n{input}\n```"));
        assert!(outcome.calls.is_empty(), "{input}");
    }
}

#[test]
fn json_codeblock_handles_params_in_other_shapes() {
    // flat 形式只保留已定义的参数键。
    let outcome = json("```json\n{\"tool_use\": \"read\", \"file_path\": \"/flat\"}\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("/flat")
    );
    // 非对象形式的 `params` 无法映射到已定义参数，整段丢弃。
    let outcome = json("```json\n{\"tool_use\": \"read\", \"params\": \"oops\"}\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert!(!outcome.calls[0].params.contains_key("params"));
    assert!(outcome.calls[0].params.is_empty());
}

#[test]
fn bare_fence_with_non_json_content_yields_no_calls() {
    let parser = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    let outcome = parser
        .try_parse("```\nplain text, not json\n```", &EnabledToolSet::all())
        .unwrap();
    assert!(outcome.calls.is_empty());
}

#[test]
fn json_codeblock_without_closing_fence_uses_rest_of_input() {
    let outcome = json("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn json_codeblock_array_skips_non_object_items() {
    let outcome = json("```json\n[42]\n```");
    assert!(outcome.calls.is_empty());

    let outcome =
        json("```json\n[42, {\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}]\n```");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn registry_auto_detect_tries_both_formats() {
    let registry = FormatRegistry::new();
    let xml_calls = registry
        .parse("<read><file_path>/a</file_path></read>")
        .unwrap()
        .calls;
    assert_eq!(xml_calls.len(), 1);

    let json_calls = registry
        .parse("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}\n```")
        .unwrap()
        .calls;
    assert_eq!(json_calls.len(), 1);
}

#[test]
fn registry_fixed_mode_switches_parser() {
    let registry = FormatRegistry::new();
    registry
        .set_mode(RegistryMode::Fixed(ToolCallFormat::JsonCodeblock))
        .unwrap();
    let template = registry
        .render_tool_call_template(&ToolKind::Write)
        .unwrap();
    assert!(template.contains("\"tool_use\": \"write\""));
    assert_eq!(registry.mode().unwrap().label(), "json-codeblock");
}

#[test]
fn registry_parse_with_unknown_format_errors() {
    let registry = FormatRegistry::new();
    assert!(registry.parse_with("bogus", "x").is_err());
}

#[test]
fn registry_auto_detect_reports_parse_errors_when_all_fail() {
    let registry = FormatRegistry::new();
    // XML yields no calls; JSON fails to parse → the registry surfaces the
    // JSON parse error instead of silently returning empty.
    // XML 不产生调用；JSON 解析失败 → 注册表返回 JSON 解析错误而非静默空结果。
    let error = registry.parse("```json\nnot json\n```").unwrap_err();
    assert!(error.message.contains("JSON parse error"));
}

#[test]
fn registry_auto_detect_returns_empty_for_plain_text() {
    let registry = FormatRegistry::new();
    let outcome = registry.parse("no tool calls here").unwrap();
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn registry_set_enabled_tools_restricts_parsing() {
    let registry = FormatRegistry::new();
    registry.set_enabled_tools(&["read".to_string()]).unwrap();
    // 禁用的 write/shell 在两种格式下都被丢弃。
    assert!(
        registry
            .parse("<write><file_path>/a</file_path></write>")
            .unwrap()
            .calls
            .is_empty()
    );
    assert!(
        registry
            .parse("```json\n{\"tool_use\": \"shell\", \"params\": {\"command\": \"ls\"}}\n```")
            .unwrap()
            .calls
            .is_empty()
    );
    assert_eq!(
        registry
            .parse("<read><file_path>/a</file_path></read>")
            .unwrap()
            .calls
            .len(),
        1
    );
}

#[test]
fn registry_unset_uses_all_tools() {
    let registry = FormatRegistry::new();
    assert_eq!(
        registry
            .parse("<shell><command>ls</command></shell>")
            .unwrap()
            .calls
            .len(),
        1
    );
}

#[test]
fn registry_set_enabled_tools_ignores_unknown_names() {
    let registry = FormatRegistry::new();
    registry
        .set_enabled_tools(&["read".to_string(), "bogus".to_string()])
        .unwrap();
    assert_eq!(
        registry
            .parse("<read><file_path>/a</file_path></read>")
            .unwrap()
            .calls
            .len(),
        1
    );
    assert!(
        registry
            .parse("<shell><command>ls</command></shell>")
            .unwrap()
            .calls
            .is_empty()
    );
}

#[test]
fn registry_default_and_auto_render() {
    let registry = FormatRegistry::default();
    let template = registry.render_tool_call_template(&ToolKind::Read).unwrap();
    assert!(template.contains("<read>"));
}

#[test]
fn parse_error_display_omits_absent_fields() {
    let error = manualaid_core::parser::ParseError::new("simple");
    assert_eq!(error.to_string(), "simple");
    let error = manualaid_core::parser::ParseError::new("x")
        .with_offset(3)
        .with_cause("deep");
    assert!(error.to_string().contains("at offset 3"));
    assert!(error.to_string().contains("deep"));
}

#[test]
fn parsers_report_their_format_names() {
    let xml = manualaid_core::parser::xml::XmlParser;
    let json = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    assert_eq!(xml.format_name(), "xml");
    assert_eq!(json.format_name(), "json-codeblock");
}

#[test]
fn xml_keeps_cdata_content_verbatim() {
    let outcome =
        xml("<write><file_path>/f</file_path><content><![CDATA[a<b & c]]></content></write>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(|v| v.as_str()),
        Some("a<b & c")
    );
}

#[test]
fn xml_cdata_mid_value_is_kept_literal() {
    // 参数值中间（非紧贴开始标签）的 `<![CDATA[` 按普通文本保留。
    let outcome = xml(
        "<write><file_path>/f</file_path><content>before <![CDATA[x]]> after</content></write>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(|v| v.as_str()),
        Some("before <![CDATA[x]]> after")
    );
}

#[test]
fn xml_cdata_content_with_template_examples_is_kept_whole() {
    // 用户场景：CDATA 里包含 write 模板示例（`<![CDATA[...]]>` 提示与
    // `<content>`/`</content>`）时，内容不应在模板示例处被截断。
    let outcome = xml(
        "<write><file_path>TEST.md</file_path><content><![CDATA[# 系统提示词\nwrap values in <![CDATA[...]]>\n<write><content>${content}</content></write>\nEND]]></content></write>",
    );
    let content = outcome.calls[0]
        .params
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(content.contains("wrap values in <![CDATA[...]]>"));
    assert!(content.contains("<write><content>${content}</content></write>"));
    assert!(content.ends_with("END"));
}

#[test]
fn xml_decodes_entity_references_in_text() {
    let outcome =
        xml("<write><file_path>/f</file_path><content>&lt;tag&gt; &amp; &#65;</content></write>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(|v| v.as_str()),
        Some("<tag> & A")
    );
}

#[test]
fn xml_tolerates_comments_between_tool_calls() {
    let outcome = xml("<read><file_path>/a</file_path></read><!-- keep going -->");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn xml_doc_example_stray_cdata_noise_before_tool() {
    // 文档示例 1：散落的 `<![CDATA[` / `]]>` 噪声不应吞掉后续的 `<edit>`
    // 调用，也不应导致"未识别到任何工具调用"。
    let outcome = xml("<![CDATA[ <![CDATA[ <![CDATA[\n\
         <![CDATA[...]]>\n\
         <![CDATA[\n\
         <edit>]]>\n\
           <file_path>README.md</file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "edit");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_cdata_after_tool_start_is_ignored() {
    // 文档示例 2：紧跟工具开始标签的 `<![CDATA[` 是噪声，不影响后续参数。
    let outcome = xml("<edit><![CDATA[\n\
           <file_path>README.md</file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_ragged_close_inside_param_value() {
    // 文档示例 3：非包裹场景的 `]]>` 在参数内按原文字面保留。
    let outcome = xml("<edit><![CDATA[\n\
           <file_path>]]>README.md</file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("]]>README.md")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_cdata_mid_value_keeps_close_tag_working() {
    // 文档示例 4：参数值中间的 `<![CDATA[` 按字面推进，`</参数>` 仍正常
    // 闭合，闭合标签之后的 `]]>` 是噪声。
    let outcome = xml("<edit>\n\
           <file_path>README.md<![CDATA[</file_path>]]>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("README.md<![CDATA[")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_cdata_wrapping_works() {
    // 文档示例 5：参数开始标签紧贴 `<![CDATA[` 时正常包裹解析。
    let outcome = xml("<edit>\n\
           <file_path><![CDATA[README.md]]></file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_cdata_with_leading_space_is_literal() {
    // 文档示例 6：参数开始标签与 `<![CDATA[` 之间有空格就不算包裹，CDATA
    // 标记连同内容按字面保留。
    let outcome = xml("<edit>\n\
           <file_path> <![CDATA[README.md]]></file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some(" <![CDATA[README.md]]>")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
}

#[test]
fn xml_doc_example_cdata_close_with_whitespace_warns() {
    // 文档示例 7：`]]>` 未紧贴参数闭合标签时 CDATA 包裹失败并累积到
    // EOF，file_path 参数标签未正确闭合，产生软警告而非静默丢弃调用。
    let outcome = xml("<edit>\n\
           <file_path><![CDATA[README.md]]> </file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<file_path>"));
    assert!(outcome.warnings[0].contains("<edit>"));
}

#[test]
fn xml_doc_example_cdata_with_spaces_is_literal() {
    // 文档示例 8：参数开始标签与 `<![CDATA[` 之间有空格，`]]>` 与闭合
    // 标签之间也有空格：不构成包裹，CDATA 标记连同两侧空格按字面保留。
    let outcome = xml("<edit>\n\
           <file_path> <![CDATA[README.md]]> </file_path>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         </edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some(" <![CDATA[README.md]]> ")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
    assert!(outcome.warnings.is_empty());
}

#[test]
fn xml_doc_example_cdata_noise_around_tool() {
    // 文档示例 9：工具开始标签后、参数之间及工具闭合标签前的散落 CDATA
    // 标记都是噪声，不影响参数解析。
    let outcome = xml("<edit><![CDATA[\n\
           <file_path>README.md</file_path>]]>\n\
           <old_string>abcdefg,hijklmn</old_string>\n\
         ]]></edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(|v| v.as_str()),
        Some("README.md")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("abcdefg,hijklmn")
    );
    assert!(outcome.warnings.is_empty());
}

#[test]
fn xml_cdata_close_must_touch_closing_tag() {
    // 严格紧跟：`]]>` 与闭合标签之间有空白就不结束 CDATA，内容原文累积
    // 到 EOF，参数标签视为未正确闭合：工具调用不识别，写入软警告。
    let outcome = xml("<write><file_path>/f</file_path><content><![CDATA[x]]>\n</content></write>");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<content>"));
    let outcome = xml("<write><file_path>/f</file_path><content><![CDATA[x]]> </content></write>");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<content>"));
}

#[test]
fn xml_comments_inside_param_are_ignored() {
    // 注释一律忽略（含参数内），不会进入参数值。
    let outcome = xml("<edit><old_string>a<!--x-->b</old_string><new_string>c</new_string></edit>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(|v| v.as_str()),
        Some("ab")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("new_string")
            .and_then(|v| v.as_str()),
        Some("c")
    );
}
