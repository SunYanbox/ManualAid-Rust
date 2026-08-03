use manualaid_core::error::CoreError;

/// All variants render readable Display strings.
/// 所有变体都可渲染可读的 Display 字符串。
#[test]
fn display_formats_all_variants() {
    let cases = [
        (CoreError::Io("io".to_string()), "IO error: io"),
        (CoreError::Config("cfg".to_string()), "config error: cfg"),
        (CoreError::NotFound("nf".to_string()), "not found: nf"),
        (
            CoreError::PermissionDenied("pd".to_string()),
            "permission denied: pd",
        ),
        (CoreError::Parse("p".to_string()), "parse error: p"),
        (CoreError::InvalidPath("ip".to_string()), "invalid path: ip"),
        (CoreError::Filter("f".to_string()), "filter error: f"),
        (CoreError::Other("o".to_string()), "Other error: o"),
        (
            CoreError::Execution {
                command: "echo".to_string(),
                exit_code: 3,
                stderr: "err".to_string(),
            },
            "command `echo` exited with code 3: err",
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
    }
}

/// `io::Error` maps to the matching `CoreError` variant by kind.
/// `io::Error` 按错误类型映射到对应的 `CoreError` 变体。
#[test]
fn io_error_conversion_maps_kinds() {
    let not_found: CoreError = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
    assert!(matches!(not_found, CoreError::NotFound(_)));
    let denied: CoreError =
        std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied").into();
    assert!(matches!(denied, CoreError::PermissionDenied(_)));
    let other: CoreError = std::io::Error::new(std::io::ErrorKind::InvalidData, "data").into();
    assert!(matches!(other, CoreError::Io(_)));
}

/// TOML deserialization failures convert to `Config` errors.
/// TOML 反序列化失败转换为 `Config` 错误。
#[test]
fn toml_deserialize_error_converts_to_config() {
    let error: CoreError = toml::from_str::<i32>("not a number").unwrap_err().into();
    assert!(matches!(error, CoreError::Config(_)));
}

/// TOML serialization failures (a top-level array of primitives is not a
/// valid TOML document) convert to `Config` errors.
/// TOML 序列化失败（顶层原始类型数组不是合法 TOML 文档）转换为 `Config` 错误。
#[test]
fn toml_serialize_error_converts_to_config() {
    let error: CoreError = toml::to_string(&vec![1, 2, 3]).unwrap_err().into();
    assert!(matches!(error, CoreError::Config(_)));
}

/// `CoreError` survives a serde round trip.
/// `CoreError` 可完成 serde 序列化往返。
#[test]
fn serde_roundtrip_preserves_error() {
    let error = CoreError::Execution {
        command: "cmd".to_string(),
        exit_code: 1,
        stderr: "err".to_string(),
    };
    let json = serde_json::to_string(&error).expect("serialize should succeed");
    let back: CoreError = serde_json::from_str(&json).expect("deserialize should succeed");
    assert_eq!(error.to_string(), back.to_string());
}
