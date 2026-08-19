use super::*;

#[test]
fn tolerates_dangling_ampersand() {
    let outcome = parse("<read><file_path>/a & b</file_path></read>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some("/a & b")
    );
}

#[test]
fn decodes_entity_references_in_values() {
    let outcome = parse("<read><file_path>a &lt; b &amp; c &#38; d &#x26; e</file_path></read>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("file_path")
            .and_then(Value::as_str),
        Some("a < b & c & d & e")
    );
}

#[test]
fn decode_text_predefined_and_numeric() {
    assert_eq!(decode_text("&amp;"), "&");
    assert_eq!(decode_text("&lt;"), "<");
    assert_eq!(decode_text("&gt;"), ">");
    assert_eq!(decode_text("&quot;"), "\"");
    assert_eq!(decode_text("&apos;"), "'");
    assert_eq!(decode_text("&#38;"), "&");
    assert_eq!(decode_text("&#x26;"), "&");
}

#[test]
fn decode_text_keeps_bare_amp_and_unknown_ref() {
    assert_eq!(decode_text("a & b"), "a & b");
    assert_eq!(decode_text("&unknown;"), "&unknown;");
    assert_eq!(decode_text(""), "");
}

#[test]
fn decode_text_numeric_multibyte_and_invalid() {
    // 多字节字符解码；无对应 XML 字符的数字引用按字面保留。
    assert_eq!(decode_text("&#x4E2D;"), "中");
    assert_eq!(decode_text("&#20013;"), "中");
    assert_eq!(decode_text("&#0;"), "&#0;");
    assert_eq!(decode_text("&#xD800;"), "&#xD800;");
    assert_eq!(decode_text("&#x110000;"), "&#x110000;");
}
