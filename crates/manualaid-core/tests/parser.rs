//! Integration tests for the parser layer: XML and JSON-codeblock parsing,
//! template rendering and the format registry modes.
//! 解析器层集成测试：XML 与 JSON-codeblock 解析、模板渲染与格式注册表
//! 模式。

use manualaid_core::parser::{FormatRegistry, RegistryMode, ToolCallFormatParser};
use manualaid_core::tools::{ToolCallFormat, ToolKind};

#[test]
fn xml_parses_and_renders_template() {
    let parser = manualaid_core::parser::xml::XmlParser;
    let calls = parser
        .try_parse("<read><file_path>/tmp/a.txt</file_path><offset>5</offset></read>")
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "read");
    assert_eq!(calls[0].format, ToolCallFormat::Xml);
    assert_eq!(
        calls[0].params.get("offset").and_then(|v| v.as_str()),
        Some("5")
    );

    let template = parser.tool_call_template(&ToolKind::Edit);
    assert!(template.contains("<edit>"));
    assert!(template.contains("<file_path>"));
}

#[test]
fn xml_keeps_parameter_order() {
    let parser = manualaid_core::parser::xml::XmlParser;
    let calls = parser
        .try_parse("<edit><new_string>B</new_string><old_string>A</old_string><file_path>/f</file_path></edit>")
        .unwrap();
    let keys: Vec<&str> = calls[0].params.keys().map(String::as_str).collect();
    assert_eq!(keys, ["new_string", "old_string", "file_path"]);
}

#[test]
fn xml_tolerates_dangling_ampersand() {
    let parser = manualaid_core::parser::xml::XmlParser;
    let calls = parser
        .try_parse("<write><file_path>/f</file_path><content>a & b</content></write>")
        .unwrap();
    assert_eq!(
        calls[0].params.get("content").and_then(|v| v.as_str()),
        Some("a & b")
    );
}

#[test]
fn json_codeblock_parses_object_and_array() {
    let parser = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    let calls = parser
        .try_parse("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}\n```")
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "read");
    assert_eq!(calls[0].format, ToolCallFormat::JsonCodeblock);

    let calls = parser
        .try_parse("```json\n[{\"tool_use\": \"write\", \"params\": {\"file_path\": \"/a\"}}]\n```")
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "write");
}

#[test]
fn json_codeblock_parses_inline_json() {
    let parser = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    let calls = parser
        .try_parse("{\"tool_use\": \"shell\", \"params\": {\"command\": \"ls\"}}")
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "shell");
}

#[test]
fn malformed_json_returns_structured_error() {
    let parser = manualaid_core::parser::json_codeblock::JsonCodeblockParser;
    let error = parser.try_parse("```json\nnot json\n```").unwrap_err();
    assert!(error.message.contains("JSON parse error"));
    assert_eq!(error.format, Some(ToolCallFormat::JsonCodeblock));
    assert!(error.to_string().contains("offset"));
}

#[test]
fn registry_auto_detect_tries_both_formats() {
    let registry = FormatRegistry::new();
    let xml_calls = registry
        .parse("<read><file_path>/a</file_path></read>")
        .unwrap();
    assert_eq!(xml_calls.len(), 1);

    let json_calls = registry
        .parse("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a\"}}\n```")
        .unwrap();
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
    let calls = registry.parse("no tool calls here").unwrap();
    assert!(calls.is_empty());
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
