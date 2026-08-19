use super::*;

#[test]
fn parses_simple_tool_call() {
    let outcome = parse(
        "<invoke name=\"read\">\n  <parameter name=\"file_path\">/test/file.txt</parameter>\n</invoke>",
    );
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
        "<invoke name=\"read\"><parameter name=\"file_path\">/a.txt</parameter></invoke><invoke name=\"read\"><parameter name=\"file_path\">/b.txt</parameter></invoke>",
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
    let outcome = parse("<invoke name=\"read\"><parameter name=\"file_path\"/></invoke>");
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
fn unknown_tool_is_ignored() {
    let outcome = parse("<invoke name=\"nonsense\"><parameter name=\"x\">1</parameter></invoke>");
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn unknown_param_is_ignored() {
    let outcome = parse(
        "<invoke name=\"read\"><parameter name=\"file_path\">/a</parameter><parameter name=\"bogus\">zzz</parameter></invoke>",
    );
    assert!(!outcome.calls[0].params.contains_key("bogus"));
}

#[test]
fn parameter_order_preserved() {
    let outcome = parse(
        "<invoke name=\"edit\"><parameter name=\"new_string\">B</parameter><parameter name=\"old_string\">A</parameter><parameter name=\"file_path\">/f</parameter></invoke>",
    );
    let keys: Vec<&str> = outcome.calls[0].params.keys().map(String::as_str).collect();
    assert_eq!(keys, ["new_string", "old_string", "file_path"]);
}

#[test]
fn template_contains_invoke_and_parameter_tags() {
    let template = InvokeParser.tool_call_template(&ToolKind::Read);
    assert!(template.contains("<invoke name=\"read\">"));
    assert!(template.contains("<parameter name=\"file_path\">"));
    assert!(template.contains("</invoke>"));
}

#[test]
fn doc_example_from_todo() {
    // TODO.md 中的示例
    let outcome = parse(
        "<invoke name=\"shell\">\n<parameter name=\"command\">git ls-files -- \"*.md\" \"doc\"</parameter>\n<parameter name=\"description\">列出仓库中所有 Markdown 文件与 doc 目录内容</parameter>\n</invoke>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "shell");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("command")
            .and_then(Value::as_str),
        Some("git ls-files -- \"*.md\" \"doc\"")
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("description")
            .and_then(Value::as_str),
        Some("列出仓库中所有 Markdown 文件与 doc 目录内容")
    );
}

#[test]
fn format_name_is_invoke() {
    assert_eq!(InvokeParser.format_name(), "invoke");
}
