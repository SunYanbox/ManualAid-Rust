use super::*;

#[test]
fn missing_name_attr_on_invoke_is_ignored() {
    let outcome = parse("<invoke><parameter name=\"file_path\">/a</parameter></invoke>");
    assert!(outcome.calls.is_empty());
}

#[test]
fn missing_name_attr_on_parameter_is_ignored() {
    let outcome = parse("<invoke name=\"read\"><parameter>/a</parameter></invoke>");
    assert!(outcome.calls.is_empty());
}

#[test]
fn single_quoted_name_attr_works() {
    let outcome = parse("<invoke name='read'><parameter name='file_path'>/a</parameter></invoke>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some("/a")
    );
}

#[test]
fn extract_name_attr_finds_name() {
    let input = " name=\"read\" other=\"x\"";
    let name = extract_name_attr(input, 0, input.len() + 1);
    assert_eq!(name.as_deref(), Some("read"));
}

#[test]
fn extract_name_attr_single_quote() {
    let input = " name='read'";
    let name = extract_name_attr(input, 0, input.len() + 1);
    assert_eq!(name.as_deref(), Some("read"));
}

#[test]
fn extract_name_attr_missing() {
    let input = " other=\"x\"";
    let name = extract_name_attr(input, 0, input.len() + 1);
    assert!(name.is_none());
}

#[test]
fn extract_name_attr_with_entity() {
    let input = " name=\"a&b\"";
    let name = extract_name_attr(input, 0, input.len() + 1);
    assert_eq!(name.as_deref(), Some("a&b"));
}

#[test]
fn extract_name_attr_space_around_equals() {
    let input = r#" name = "read""#;
    assert_eq!(
        extract_name_attr(input, 0, input.len() + 1).as_deref(),
        Some("read")
    );
}

#[test]
fn extract_name_attr_whitespace_only_is_none() {
    assert!(extract_name_attr("   ", 0, 4).is_none());
}

#[test]
fn extract_name_attr_without_equals_is_none() {
    assert!(extract_name_attr(" name", 0, 6).is_none());
}

#[test]
fn extract_name_attr_without_quote_is_none() {
    assert!(extract_name_attr(" name = ", 0, 9).is_none());
}

#[test]
fn extract_name_attr_unquoted_value_is_none() {
    assert!(extract_name_attr(" name=read", 0, 11).is_none());
}
