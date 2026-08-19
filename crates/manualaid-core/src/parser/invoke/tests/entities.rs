use super::*;

#[test]
fn entity_references_decoded() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\"><tag> & A</parameter></invoke>",
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("<tag> & A")
    );
}

#[test]
fn bare_ampersand_kept_verbatim() {
    let outcome = parse(
        "<invoke name=\"write\"><parameter name=\"file_path\">/f</parameter><parameter name=\"content\">a & b</parameter></invoke>",
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("a & b")
    );
}
