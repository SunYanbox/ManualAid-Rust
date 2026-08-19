use super::*;

#[test]
fn coerce_integer_strings() {
    let mut params = IndexMap::new();
    params.insert("offset".to_string(), Value::String("60".into()));
    params.insert("limit".to_string(), Value::String("1.0".into()));
    let coerced = coerce_params(&params, ToolKind::Read);
    assert_eq!(coerced.get("offset").and_then(Value::as_i64), Some(60));
    assert_eq!(coerced.get("limit").and_then(Value::as_i64), Some(1));
}

#[test]
fn coerce_leaves_unparseable_strings_untouched() {
    let mut params = IndexMap::new();
    params.insert("offset".to_string(), Value::String("abc".into()));
    let coerced = coerce_params(&params, ToolKind::Read);
    assert_eq!(coerced.get("offset").and_then(Value::as_str), Some("abc"));
}

#[test]
fn coerce_leaves_non_string_values_untouched() {
    let mut params = IndexMap::new();
    params.insert("offset".to_string(), Value::Bool(true));
    let coerced = coerce_params(&params, ToolKind::Read);
    assert_eq!(coerced.get("offset"), Some(&Value::Bool(true)));
}

#[test]
fn coerce_string_converts_generic_kinds() {
    assert_eq!(coerce_string("number", "1.5"), Some(Value::from(1.5)));
    assert_eq!(coerce_string("float", "abc"), None);
    assert_eq!(coerce_string("boolean", "TRUE"), Some(Value::Bool(true)));
    assert_eq!(coerce_string("boolean", "false"), Some(Value::Bool(false)));
    assert_eq!(coerce_string("boolean", "maybe"), None);
    assert!(coerce_string("object", "{\"a\":1}").unwrap().is_object());
    assert!(coerce_string("object", "not-json").is_none());
    assert!(coerce_string("array", "[1,2]").unwrap().is_array());
    assert!(coerce_string("array", "x").is_none());
    assert_eq!(coerce_string("untyped", "anything"), None);
}
