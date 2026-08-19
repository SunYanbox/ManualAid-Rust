use super::*;

#[test]
fn json_type_names_are_stable() {
    assert_eq!(json_type_name(&Value::Null), "null");
    assert_eq!(json_type_name(&Value::Bool(true)), "boolean");
    assert_eq!(json_type_name(&Value::from(1)), "integer");
    assert_eq!(json_type_name(&Value::from(1.5)), "float");
    assert_eq!(json_type_name(&Value::String("s".into())), "string");
    assert_eq!(json_type_name(&Value::Array(vec![])), "array");
    assert_eq!(json_type_name(&Value::Object(Default::default())), "object");
}
