use super::*;

#[test]
fn template_contains_optional_marker_and_escape_hint() {
    let template = XmlParser.tool_call_template(&crate::parser::ToolTemplate::from_kind(
        crate::tools::ToolKind::Read,
    ));
    assert!(template.contains("<read>"));
    assert!(template.contains("<!-- optional -->"));
    assert!(template.contains("CDATA"));
}
