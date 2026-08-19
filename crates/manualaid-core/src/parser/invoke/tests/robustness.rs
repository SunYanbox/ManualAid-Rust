use super::*;

#[test]
fn self_closing_tool_is_ignored() {
    let outcome = parse("<invoke name=\"read\"/>");
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn nested_tool_inside_call_is_ignored() {
    let outcome = parse(
        "<invoke name=\"read\"><parameter name=\"file_path\">/a</parameter><invoke name=\"edit\"><parameter name=\"new_string\">y</parameter></invoke></invoke>",
    );
    assert!(outcome.calls[0].params.contains_key("file_path"));
    assert!(!outcome.calls[0].params.contains_key("new_string"));
}

#[test]
fn unclosed_param_discarded_with_warning() {
    let outcome = parse("<invoke name=\"edit\"><parameter name=\"old_string\">x</invoke>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "edit");
    assert!(!outcome.calls[0].params.contains_key("old_string"));
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("old_string"));
}

#[test]
fn unclosed_tool_is_ignored() {
    let outcome = parse("<invoke name=\"edit\"><parameter name=\"old_string\">x</parameter>");
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn bare_less_than_in_param_value_is_literal() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\">p < target</parameter></invoke>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("p < target")
    );
    assert!(outcome.warnings.is_empty());
}

#[test]
fn param_value_keeps_other_tags_literal() {
    let outcome = parse(
        "<invoke name=\"edit\"><parameter name=\"file_path\">/f</parameter><parameter name=\"old_string\">if <read> then</parameter><parameter name=\"new_string\">ok</parameter></invoke>",
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
        "<invoke name=\"read\"><parameter name=\"file_path\">/a</parameter></invoke><indent>  </indent><invoke name=\"edit\"><parameter name=\"file_path\">/b</parameter><parameter name=\"old_string\">x</parameter><parameter name=\"new_string\">y</parameter></invoke>",
    );
    assert_eq!(outcome.calls.len(), 2);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(outcome.calls[1].tool_name, "edit");
}

#[test]
fn ignores_unknown_wrapper_elements() {
    let outcome = parse(
        "<wrapper><invoke name=\"read\"><parameter name=\"file_path\">/a</parameter></invoke></wrapper>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn unmatched_close_tag_is_ignored() {
    let outcome = parse(
        "<invoke name=\"read\"><parameter name=\"file_path\">/a</parameter></invoke></wrapper>",
    );
    assert_eq!(outcome.calls.len(), 1);
}

#[test]
fn uppercase_tags_are_ignored() {
    let outcome =
        parse("<INVOKE name=\"read\"><PARAMETER name=\"file_path\">/a</PARAMETER></INVOKE>");
    assert!(outcome.calls.is_empty());
}

#[test]
fn comments_inside_param_are_ignored() {
    let outcome = parse(
        "<invoke name=\"edit\"><parameter name=\"old_string\">a<!--x-->b</parameter><parameter name=\"new_string\">c</parameter></invoke>",
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(Value::as_str),
        Some("ab")
    );
}

#[test]
fn invoke_tag_in_unknown_wrapper_does_not_break_later_calls() {
    let outcome = parse(
        "<intent>第38行是<invoke name=\"read\">工具模板中的注释行</intent>\n<invoke name=\"shell\"><parameter name=\"command\">echo hi</parameter></invoke>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "shell");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("command")
            .and_then(Value::as_str),
        Some("echo hi")
    );
}

#[test]
fn trailing_bare_less_than_in_param_is_discarded() {
    let outcome = parse(
        r#"<invoke name="write"><parameter name="file_path">/f</parameter><parameter name="content">abc<"#,
    );
    assert!(outcome.calls.is_empty());
    assert!(outcome.warnings.is_empty());
}

#[test]
fn self_closing_nested_invoke_resets_outer_state() {
    let outcome = parse(
        r#"<invoke name="read"><parameter name="file_path">/a</parameter><invoke name="read"/></invoke>"#,
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some("/a")
    );
    assert!(outcome.warnings.is_empty());
}
