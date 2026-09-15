use serde_json::json;

use super::*;

#[test]
fn exposed_name_prefixes_the_server() {
    assert_eq!(exposed_name("fs", "read_file"), "mcp_fs_read_file");
}

#[test]
fn exposed_name_keeps_underscores_in_both_parts() {
    // The exposed name is matched whole, so underscores inside either part
    // stay unambiguous and must be preserved verbatim.
    // 暴露名按整体匹配，因此任一部分内的下划线都不会产生歧义，必须原样保留。
    assert_eq!(
        exposed_name("my_server", "my_tool"),
        "mcp_my_server_my_tool"
    );
}

#[test]
fn a_schema_without_object_properties_yields_no_params() {
    assert!(parse_params(None).is_empty());
    assert!(parse_params(Some(&json!({"type": "object"}))).is_empty());
    assert!(parse_params(Some(&json!("string"))).is_empty());
    assert!(parse_params(Some(&json!({"properties": []}))).is_empty());
}

#[test]
fn required_names_mark_the_matching_params() {
    let schema = json!({
        "type": "object",
        "properties": {"path": {"type": "string"}, "depth": {"type": "integer"}},
        "required": ["path"]
    });
    let params = parse_params(Some(&schema));
    assert_eq!(params.len(), 2);
    assert!(param(&params, "path").required);
    assert!(!param(&params, "depth").required);
}

#[test]
fn schema_types_map_onto_kind_hints() {
    let schema = json!({
        "type": "object",
        "properties": {
            "s": {"type": "string"},
            "i": {"type": "integer"},
            "n": {"type": "number"},
            "b": {"type": "boolean"},
            "a": {"type": "array"},
            "o": {"type": "object"}
        }
    });
    let params = parse_params(Some(&schema));
    for (name, kind) in [
        ("s", "string"),
        ("i", "integer"),
        ("n", "number"),
        ("b", "boolean"),
        ("a", "array"),
        ("o", "object"),
    ] {
        assert_eq!(param(&params, name).kind, kind, "{name}");
    }
}

#[test]
fn unusable_type_declarations_fall_back_to_string() {
    // The hint only drives display and best-effort coercion, so a schema that
    // omits or generalises the type must not lose the parameter.
    // 该提示只影响展示与尽力而为的强制转换，因此省略或泛化类型的 schema
    // 不应丢失该参数。
    let schema = json!({
        "type": "object",
        "properties": {
            "missing": {},
            "multi": {"type": ["string", "null"]},
            "numeric": {"type": 7}
        }
    });
    let params = parse_params(Some(&schema));
    assert_eq!(params.len(), 3);
    for name in ["missing", "multi", "numeric"] {
        assert_eq!(param(&params, name).kind, "string", "{name}");
    }
}

#[test]
fn descriptions_are_carried_over_and_default_to_empty() {
    let schema = json!({
        "type": "object",
        "properties": {
            "documented": {"type": "string", "description": "reads a file"},
            "undocumented": {"type": "string"},
            "non_string": {"type": "string", "description": 7}
        }
    });
    let params = parse_params(Some(&schema));
    assert_eq!(param(&params, "documented").description, "reads a file");
    assert_eq!(param(&params, "undocumented").description, "");
    assert_eq!(param(&params, "non_string").description, "");
}

#[test]
fn parse_tool_fills_every_field() {
    let schema = json!({"type": "object", "properties": {"path": {"type": "string"}}});
    let tool = parse_tool("fs", "read_file", "reads a file", Some(&schema));
    assert_eq!(tool.server_name, "fs");
    assert_eq!(tool.tool_name, "read_file");
    assert_eq!(tool.exposed_name, "mcp_fs_read_file");
    assert_eq!(tool.description, "reads a file");
    assert_eq!(tool.params.len(), 1);
    assert_eq!(tool.params[0].name, "path");
}

#[test]
fn parse_tool_accepts_a_tool_without_a_schema() {
    let tool = parse_tool("fs", "ping", "", None);
    assert_eq!(tool.exposed_name, "mcp_fs_ping");
    assert!(tool.params.is_empty());
}

/// Look up a parsed parameter by name, so assertions do not depend on the
/// order `serde_json` happens to iterate object keys in.
/// 按名称查找已解析的参数，使断言不依赖 `serde_json` 遍历对象键的顺序。
fn param<'a>(params: &'a [McpParam], name: &str) -> &'a McpParam {
    params
        .iter()
        .find(|param| param.name == name)
        .unwrap_or_else(|| panic!("no parameter named `{name}`"))
}
