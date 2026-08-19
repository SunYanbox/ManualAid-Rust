use super::*;

#[test]
fn auto_detect_finds_xml_calls() {
    let registry = FormatRegistry::new();
    let calls = registry
        .parse("<read><file_path>/a.txt</file_path></read>")
        .unwrap()
        .calls;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].tool_name, "read");
}

#[test]
fn auto_detect_finds_json_calls() {
    let registry = FormatRegistry::new();
    let calls = registry
        .parse("```json\n{\"tool_use\": \"read\", \"params\": {\"file_path\": \"/a.txt\"}}\n```")
        .unwrap()
        .calls;
    assert_eq!(calls.len(), 1);
}

#[test]
fn fixed_xml_mode_ignores_json_input() {
    let registry = FormatRegistry::new();
    registry
        .set_mode(RegistryMode::Fixed(ToolCallFormat::Xml))
        .unwrap();
    let calls = registry
        .parse("{\"tool_use\": \"read\", \"params\": {}}")
        .unwrap()
        .calls;
    assert!(calls.is_empty());
}

#[test]
fn mode_labels_round_trip() {
    for &label in RegistryMode::all_labels() {
        let mode = RegistryMode::from_label(label).unwrap();
        assert_eq!(mode.label(), label);
    }
    assert!(RegistryMode::from_label("bogus").is_none());
}
