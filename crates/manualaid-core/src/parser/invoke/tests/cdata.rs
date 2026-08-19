use super::*;

#[test]
fn cdata_content_kept_verbatim() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\"><![CDATA[a<b & c]]></parameter></invoke>",
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("a<b & c")
    );
}

#[test]
fn cdata_mid_value_is_kept_literal() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\">before <![CDATA[x]]> after</parameter></invoke>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("before <![CDATA[x]]> after")
    );
}

#[test]
fn cdata_close_must_touch_closing_tag() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\"><![CDATA[x]]>\n</parameter></invoke>",
    );
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("content"));
}

#[test]
fn cdata_with_spaces_is_literal() {
    let outcome = parse(
        "<invoke name=\"edit\"><parameter name=\"file_path\"> <![CDATA[README.md]]> </parameter><parameter name=\"old_string\">abc</parameter></invoke>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some(" <![CDATA[README.md]]> ")
    );
    assert!(outcome.warnings.is_empty());
}
