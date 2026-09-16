use super::*;

#[test]
fn transport_labels_round_trip() {
    for kind in [McpTransportKind::Stdio, McpTransportKind::Http] {
        assert_eq!(McpTransportKind::from_label(kind.label()), Some(kind));
    }
}

#[test]
fn unknown_transport_labels_are_rejected() {
    // Rejecting instead of defaulting lets the configuration layer report the
    // offending value and skip only that entry.
    // 拒绝而非取默认值，使配置层能报告问题值并只跳过该条目。
    for label in ["", "sse", "Stdio", "http "] {
        assert_eq!(McpTransportKind::from_label(label), None, "{label}");
    }
}

#[test]
fn a_stdio_server_needs_a_non_blank_command() {
    let mut server = stdio("files");
    assert!(server.validate().is_ok());

    server.command = None;
    assert!(server.validate().is_err());

    server.command = Some("   ".to_string());
    assert!(server.validate().is_err());
}

#[test]
fn an_http_server_needs_a_non_blank_url() {
    let mut server = http("remote");
    assert!(server.validate().is_ok());

    server.url = None;
    assert!(server.validate().is_err());

    server.url = Some("\t".to_string());
    assert!(server.validate().is_err());
}

#[test]
fn the_server_name_must_not_be_blank() {
    let mut server = stdio("");
    assert!(server.validate().is_err());

    server.name = "   ".to_string();
    assert!(server.validate().is_err());
}

#[test]
fn fields_belonging_to_the_other_transport_are_ignored() {
    // Users copy entries around; a leftover field is not an error as long as
    // the selected transport is satisfied.
    // 用户会复制条目；只要所选传输的必需字段齐备，遗留字段不构成错误。
    let mut server = stdio("files");
    server.url = Some(String::new());
    assert!(server.validate().is_ok());

    let mut server = http("remote");
    server.command = None;
    assert!(server.validate().is_ok());
}
