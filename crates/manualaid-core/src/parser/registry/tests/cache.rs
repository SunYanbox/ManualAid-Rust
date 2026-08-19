use super::*;

#[test]
fn set_enabled_tools_reuses_cache_for_same_set() {
    let registry = FormatRegistry::new();
    registry
        .set_enabled_tools(&["read".to_string(), "edit".to_string()])
        .unwrap();
    let first = registry.enabled_tool_set().unwrap();
    // 乱序的相同集合不重建缓存。
    registry
        .set_enabled_tools(&["edit".to_string(), "read".to_string()])
        .unwrap();
    let second = registry.enabled_tool_set().unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn set_enabled_tools_rebuilds_when_set_changes() {
    let registry = FormatRegistry::new();
    registry.set_enabled_tools(&["read".to_string()]).unwrap();
    let first = registry.enabled_tool_set().unwrap();
    registry
        .set_enabled_tools(&["read".to_string(), "edit".to_string()])
        .unwrap();
    let second = registry.enabled_tool_set().unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn unset_registry_returns_all_tools_set() {
    let registry = FormatRegistry::new();
    let set = registry.enabled_tool_set().unwrap();
    assert_eq!(set.tool_names().len(), crate::tools::all_tools().len());
}

#[test]
fn parse_with_unknown_format_is_an_error() {
    let registry = FormatRegistry::new();
    assert!(registry.parse_with("nonexistent", "input").is_err());
}

#[test]
fn renders_template_for_current_mode() {
    let registry = FormatRegistry::new();
    registry
        .set_mode(RegistryMode::Fixed(ToolCallFormat::JsonCodeblock))
        .unwrap();
    let template = registry.render_tool_call_template(&ToolKind::Read).unwrap();
    assert!(template.contains("\"tool_use\": \"read\""));
}

#[test]
fn registered_formats_contains_builtins() {
    let formats = FormatRegistry::new().registered_formats().unwrap();
    assert!(formats.contains(&"xml".to_string()));
    assert!(formats.contains(&"json-codeblock".to_string()));
}

#[test]
fn render_template_in_fixed_mode_reports_missing_parser() {
    let registry = FormatRegistry::new();
    registry
        .set_mode(RegistryMode::Fixed(ToolCallFormat::Xml))
        .unwrap();
    // 从注册表移除解析器后，固定模式应报"未注册"错误。
    registry.parsers.write().unwrap().shift_remove("xml");
    let err = registry
        .render_tool_call_template(&ToolKind::Read)
        .expect_err("fixed mode with a removed parser must fail");
    assert!(
        err.message
            .contains("No parser registered for format `xml`")
    );
}

#[test]
fn poisoned_lock_reports_error() {
    let registry = FormatRegistry::new();
    // 在持锁时 panic 使 parsers 锁进入中毒状态，之后的操作应返回错误。
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let _guard = registry.parsers.write().unwrap();
                panic!("poison the registry lock");
            })
            .join()
            .expect_err("scoped thread must panic");
    });
    let err = registry
        .parse("<read><file_path>/a.txt</file_path></read>")
        .expect_err("poisoned lock must surface as a ParseError");
    assert_eq!(err.message, "Registry lock poisoned");
}
