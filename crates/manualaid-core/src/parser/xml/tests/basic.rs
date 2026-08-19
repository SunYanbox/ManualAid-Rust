use super::*;

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
fn empty_tool_call_is_ignored() {
    assert!(parse("<read></read>").calls.is_empty());
}

#[test]
fn tool_without_params_is_ignored() {
    // 闭合标签之间只有文本或未知标签，没有任何有效参数 → 忽略。
    assert!(parse("<read>some text only</read>").calls.is_empty());
    assert!(parse("<read><foo>x</foo></read>").calls.is_empty());
}
