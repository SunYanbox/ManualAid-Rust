use super::*;

#[test]
fn format_name_is_json_codeblock() {
    assert_eq!(JsonCodeblockParser.format_name(), "json-codeblock");
}

#[test]
fn parses_tool_call_in_func_calls_fence() {
    let calls = JsonCodeblockParser
        .try_parse(
            "```func_calls\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a.txt\"}}\n```",
            &EnabledToolSet::all(),
        )
        .unwrap()
        .calls;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "read");
    assert_eq!(
        calls[0].params.get("file_path").and_then(Value::as_str),
        Some("/a.txt")
    );
}

#[test]
fn parses_params_fallback_keys() {
    let calls = JsonCodeblockParser
        .try_parse(
            "```json\n{\"tool_use\": \"read\", \"file_path\": \"/a.txt\"}\n```",
            &EnabledToolSet::all(),
        )
        .unwrap()
        .calls;
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
            &EnabledToolSet::all(),
        )
        .unwrap()
        .calls;
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].tool_name, "edit");
}

#[test]
fn missing_tool_use_yields_no_calls() {
    let outcome = JsonCodeblockParser
        .try_parse(
            "```json\n{\"params\": {\"x\": \"1\"}}\n```",
            &EnabledToolSet::all(),
        )
        .unwrap();
    assert!(outcome.calls.is_empty());
}

#[test]
fn non_object_value_yields_no_calls() {
    let outcome = JsonCodeblockParser
        .try_parse("```json\n\"just a string\"\n```", &EnabledToolSet::all())
        .unwrap();
    assert!(outcome.calls.is_empty());
}

#[test]
fn unknown_tool_use_is_skipped() {
    let outcome = JsonCodeblockParser
        .try_parse(
            "```json\n{\"tool_use\": \"bogus\", \"params\": {\"x\": \"1\"}}\n```",
            &EnabledToolSet::all(),
        )
        .unwrap();
    assert!(outcome.calls.is_empty());
}

#[test]
fn empty_input_yields_no_calls() {
    let calls = JsonCodeblockParser
        .try_parse("", &EnabledToolSet::all())
        .unwrap()
        .calls;
    assert!(calls.is_empty());
}

#[test]
fn template_marks_optional_params() {
    let template = JsonCodeblockParser.tool_call_template(&ToolKind::Read);
    assert!(template.contains("\"tool_use\": \"read\""));
    assert!(template.contains("// optional"));
    assert!(template.contains("escape"));
}
