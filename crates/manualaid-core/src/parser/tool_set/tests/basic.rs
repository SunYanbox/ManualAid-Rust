use super::*;

#[test]
fn from_names_resolves_and_drops_unknown() {
    let set = EnabledToolSet::from_names(&["read".to_string(), "bogus".to_string()]);
    assert!(set.contains_tool("read"));
    assert!(!set.contains_tool("bogus"));
    assert!(!set.contains_tool("shell"));
}

#[test]
fn from_tool_kinds_builds_param_index() {
    let set = EnabledToolSet::from_tool_kinds(&[ToolKind::Edit]);
    assert!(set.contains_tool("edit"));
    assert!(set.contains_param("edit", "file_path"));
    assert!(set.contains_param("edit", "old_string"));
    assert!(set.contains_param("edit", "new_string"));
    assert!(set.contains_param("edit", "replace_all"));
    assert!(!set.contains_param("edit", "command"));
}

#[test]
fn default_equals_all() {
    let set = EnabledToolSet::default();
    assert_eq!(set.tool_names().len(), all_tools().len());
}

#[test]
fn contains_tool_and_param_lookups() {
    let set = EnabledToolSet::all();
    assert_eq!(set.tool_kind("read"), Some(ToolKind::Read));
    assert_eq!(set.tool_kind("nonsense"), None);
    assert!(set.contains_param("read", "file_path"));
    // 参数名区分大小写，与标签名一致。
    assert!(!set.contains_param("read", "FILE_PATH"));
}

#[test]
fn tool_names_returns_canonical_order() {
    // 乱序输入也会按 all_tools 顺序规范化，保证指纹稳定。
    let set =
        EnabledToolSet::from_names(&["skill".to_string(), "read".to_string(), "write".to_string()]);
    assert_eq!(set.tool_names(), vec!["read", "write", "skill"]);
}
