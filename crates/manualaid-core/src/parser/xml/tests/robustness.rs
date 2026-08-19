use super::*;

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
fn bare_less_than_in_param_value_is_literal() {
    // 参数值中的裸 `<` 后接空白等非法名称起始字符时应按字面文本
    // 保留，不得吞掉后续真实闭合标签。
    let outcome = parse("<write><file_path>/f</file_path><content>p < target</content></write>");
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
fn bare_less_than_before_tool_call_does_not_swallow_start_tag() {
    // 空闲文本中的裸 `<` 不应吞掉后续工具调用的开始标签。
    let outcome = parse("a < b<read><file_path>/a.txt</file_path></read>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some("/a.txt")
    );
}

#[test]
fn bare_lt_slash_in_param_kept_literal() {
    // 参数值中的 `</ `（空名闭合）应作为字面文本保留，正常闭合。
    let outcome = parse(
        "<edit><file_path>/f</file_path><old_string>a </ b</old_string><new_string>x</new_string></edit>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(Value::as_str),
        Some("a </ b")
    );
    assert!(outcome.warnings.is_empty());
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
