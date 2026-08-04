//! Integration tests for the privacy masking module: config loading from
//! real temp files, sanitize → restore round trips, and snapshot
//! serialization via `serde_json`.
//! 隐私掩码模块的集成测试：从真实临时文件加载配置、脱敏 → 还原往返，
//! 以及经 `serde_json` 的快照序列化。

use std::fs;

use manualaid_core::privacy::{
    PrivacyMaskExtension, PrivacyMasker, PrivacyRegistry, PrivacyRegistrySnapshot,
    global_privacy_registry, restore_masked_data, sanitize_prompt,
};

mod common;

#[test]
fn test_config_load_and_merge_with_temp_files() {
    let tmp = common::TempDir::new("privacy-config");
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    fs::create_dir_all(home.join(".ManualAid")).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();

    fs::write(
        home.join(".ManualAid").join("config.toml"),
        r#"
[privacy_mask_extension.regex]
A = "^a$"
B = "^b$"

[privacy_mask_extension.literal]
Name = "Alice"
Keep = "global"
"#,
    )
    .unwrap();
    fs::write(
        project.join(".ManualAid").join("config.toml"),
        r#"
[privacy_mask_extension.regex]
B = "^bb$"
C = "^c$"

[privacy_mask_extension.literal]
Name = "Bob"
"#,
    )
    .unwrap();

    let ext = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    assert_eq!(ext.regex.get("A").map(String::as_str), Some("^a$"));
    assert_eq!(ext.regex.get("B").map(String::as_str), Some("^bb$"));
    assert_eq!(ext.regex.get("C").map(String::as_str), Some("^c$"));
    assert_eq!(ext.regex.len(), 3);
    assert_eq!(ext.literal.get("Name").map(String::as_str), Some("Bob"));
    assert_eq!(ext.literal.get("Keep").map(String::as_str), Some("global"));
    assert_eq!(ext.literal.len(), 2);
}

#[test]
fn test_sanitize_restore_roundtrip_from_config() {
    let tmp = common::TempDir::new("privacy-roundtrip");
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();
    fs::write(
        project.join(".ManualAid").join("config.toml"),
        r#"
[privacy_mask_extension.regex]
ExamApiKey = "sk-[A-Za-z0-9]{7}"

[privacy_mask_extension.literal]
UserName = "Alice"
"#,
    )
    .unwrap();

    // `from_config` resolves the real user home; use explicit temp paths so
    // the test is independent of the environment.
    // `from_config` 会解析真实用户主目录；这里用显式临时路径，使测试
    // 与环境无关。
    let extensions = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    let masker = PrivacyMasker::from_extensions(&extensions).unwrap();
    let text = "C:/User/Alice/Download and key sk-114dsx6, mail jane@example.com";
    let (masked, mapping) = masker.sanitize(text).unwrap();
    assert!(!masked.contains("Alice"));
    assert!(!masked.contains("sk-114dsx6"));
    assert!(!masked.contains("jane@example.com"));
    assert!(masked.contains("[PRV_UserName_"));
    assert!(masked.contains("[PRV_ExamApiKey_"));
    assert!(masked.contains("[PRV_EMAIL_"));
    assert_eq!(restore_masked_data(&masked, &mapping), text);
}

#[test]
fn test_sanitize_prompt_builtin_only_integration() {
    let (masked, mapping) = sanitize_prompt("mail me at bob@example.com").unwrap();
    assert!(masked.contains("[PRV_EMAIL_"));
    assert!(!masked.contains("bob@example.com"));
    assert_eq!(
        restore_masked_data(&masked, &mapping),
        "mail me at bob@example.com"
    );
}

#[test]
fn test_snapshot_json_roundtrip() {
    let reg = PrivacyRegistry::new();
    let id = reg.get_or_create("EMAIL", "jane@example.com").unwrap();
    let _ = reg.get_or_create("PHONE", "13800138000").unwrap();

    let snapshot = reg.export_snapshot().unwrap();
    let json = serde_json::to_string(&snapshot).unwrap();
    // The snapshot must never contain plaintext.
    // 快照绝不能包含明文。
    assert!(!json.contains("jane@example.com"));
    assert!(!json.contains("13800138000"));

    let decoded: PrivacyRegistrySnapshot = serde_json::from_str(&json).unwrap();
    let reloaded = PrivacyRegistry::from_snapshot(decoded).unwrap();
    assert_eq!(
        reloaded.get_or_create("EMAIL", "jane@example.com").unwrap(),
        id
    );
    assert_eq!(
        reloaded
            .get_or_create("EMAIL", "other@example.com")
            .unwrap(),
        "[PRV_EMAIL_2]"
    );
}

#[test]
fn test_global_registry_snapshot_reload() {
    let snapshot = global_privacy_registry().export_snapshot().unwrap();
    let reloaded = PrivacyRegistry::from_snapshot(snapshot).unwrap();
    assert_eq!(
        reloaded.len().unwrap(),
        global_privacy_registry().len().unwrap()
    );
}

#[test]
fn test_large_text_masking_is_correct() {
    let mut extensions = PrivacyMaskExtension::default();
    extensions
        .regex
        .insert("ExamApiKey".to_string(), r"sk-[A-Za-z0-9]{7}".to_string());
    extensions
        .literal
        .insert("UserName".to_string(), "Alice".to_string());
    let masker = PrivacyMasker::from_extensions(&extensions).unwrap();

    // ~255k characters with repeated sensitive values; the pipeline must
    // stay linear-time and produce a correct round trip.
    // 约 25.5 万字符、含重复敏感值；管线必须保持线性时间且往返正确。
    let mut text = String::with_capacity(260_000);
    for _ in 0..15_000 {
        text.push_str("Alice sk-1234567 ");
    }

    let (masked, mapping) = masker.sanitize(&text).unwrap();
    assert!(!masked.contains("Alice"));
    assert!(!masked.contains("sk-1234567"));
    assert!(masked.contains("[PRV_UserName_"));
    assert!(masked.contains("[PRV_ExamApiKey_"));
    assert_eq!(restore_masked_data(&masked, &mapping), text);
}
