use super::*;

#[test]
fn validate_requires_present_params() {
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::String("/a".into()));
    let error = validate_params(&params, ToolKind::Write).unwrap_err();
    assert!(error.message.contains("content"));
}

#[test]
fn validate_accepts_valid_params() {
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::String("/a".into()));
    params.insert("offset".to_string(), Value::from(1));
    assert!(validate_params(&params, ToolKind::Read).is_ok());
    assert!(validate_params(&params, ToolKind::Write).is_err());
}

#[test]
fn validate_rejects_wrong_type() {
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::String("/a".into()));
    params.insert("offset".to_string(), Value::String("not-a-number".into()));
    let error = validate_params(&params, ToolKind::Read).unwrap_err();
    assert!(error.message.contains("expected type"));
}

#[test]
fn validate_rejects_empty_and_null_required_params() {
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::String(String::new()));
    let error = validate_params(&params, ToolKind::Read).unwrap_err();
    assert!(error.message.contains("must not be empty"));
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::Null);
    let error = validate_params(&params, ToolKind::Read).unwrap_err();
    assert!(error.message.contains("must not be null"));
}

#[test]
fn check_value_type_accepts_generic_kinds() {
    assert!(check_value_type("p", "number", &Value::from(1.5)).is_ok());
    assert!(check_value_type("p", "array[string]", &Value::Array(vec![])).is_ok());
    assert!(check_value_type("p", "object", &Value::Object(Default::default())).is_ok());
    assert!(check_value_type("p", "anything", &Value::Null).is_ok());
}
