//! API tests: input reading, masking and restoring.
//! API 测试：输入读取、掩码与恢复。

use std::collections::{BTreeMap, HashMap};
use std::fs;

use manualaid_cli::{mask, mask_with_chars, read_input, restore, restore_with_chars};
use manualaid_core::error::CoreError;
use manualaid_core::privacy::{PrivacyMaskExtension, PrivacyMasker, restore_masked_data};

use super::common;

#[test]
fn read_input_reads_existing_file() {
    let tmp = common::TempDir::new("read-file");
    let path = tmp.path().join("input.txt");
    fs::write(&path, "file content").unwrap();
    assert_eq!(read_input(path.to_str().unwrap()).unwrap(), "file content");
}

#[test]
fn read_input_directory_is_invalid_path() {
    let tmp = common::TempDir::new("read-dir");
    assert!(matches!(
        read_input(tmp.path().to_str().unwrap()),
        Err(CoreError::InvalidPath(_))
    ));
}

#[test]
fn read_input_missing_path_is_literal_text() {
    assert_eq!(read_input("not a real path").unwrap(), "not a real path");
}

#[test]
fn mask_hides_plaintext_and_roundtrips() {
    let masker = PrivacyMasker::new().unwrap();
    let input = "mail me at bob@example.com";
    let (masked, snapshot) = mask(&masker, input).unwrap();
    assert!(masked.contains("[PRV_EMAIL_"));
    assert!(!masked.contains("bob@example.com"));
    assert_eq!(snapshot.values().next().unwrap(), "bob@example.com");

    let mapping: HashMap<String, String> = snapshot.clone().into_iter().collect();
    assert_eq!(restore_masked_data(&masked, &mapping), input);
}

#[test]
fn mask_uses_config_extensions_with_fake_home() {
    let tmp = common::TempDir::new("mask-ext");
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();
    fs::write(
        project.join(".ManualAid").join("config.toml"),
        "[privacy_mask_extension.literal]\nUserName = \"Alice\"\n",
    )
    .unwrap();

    let extensions = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    let masker = PrivacyMasker::from_extensions(&extensions).unwrap();
    let (masked, snapshot) = mask(&masker, "hi Alice").unwrap();
    assert!(masked.contains("[PRV_UserName_"));
    assert!(!masked.contains("Alice"));
    let mapping: HashMap<String, String> = snapshot.into_iter().collect();
    assert_eq!(restore_masked_data(&masked, &mapping), "hi Alice");
}

#[test]
fn mask_snapshot_keys_are_sorted() {
    let masker = PrivacyMasker::new().unwrap();
    let (masked, snapshot) = mask(&masker, "a@example.com b@example.com").unwrap();
    let keys: Vec<&String> = snapshot.keys().collect();
    assert_eq!(keys.len(), 2);
    assert!(keys[0] < keys[1]);
    assert!(keys[0].starts_with("[PRV_EMAIL_"));
    assert!(keys[1].starts_with("[PRV_EMAIL_"));
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.find(keys[0]).unwrap() < json.find(keys[1]).unwrap());
    assert!(!masked.contains("a@example.com"));
    assert!(!masked.contains("b@example.com"));
}

#[test]
fn restore_roundtrip_via_snapshot_file() {
    let tmp = common::TempDir::new("restore");
    let masker = PrivacyMasker::new().unwrap();
    let input = "call +1 555 010 1234 now";
    let (masked, snapshot) = mask(&masker, input).unwrap();

    let snapshot_path = tmp.path().join("snapshot.json");
    fs::write(
        &snapshot_path,
        serde_json::to_string_pretty(&snapshot).unwrap(),
    )
    .unwrap();

    let restored = restore(&masked, &snapshot_path).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn restore_missing_snapshot_is_io_error() {
    let tmp = common::TempDir::new("restore-missing");
    let err = restore("[PRV_EMAIL_1]", &tmp.path().join("missing.json")).unwrap_err();
    assert!(matches!(err, CoreError::Io(_) | CoreError::NotFound(_)));
}

#[test]
fn restore_invalid_snapshot_json_is_parse_error() {
    let tmp = common::TempDir::new("restore-invalid");
    let path = tmp.path().join("snapshot.json");
    fs::write(&path, "not json").unwrap();
    let err = restore("[PRV_EMAIL_1]", &path).unwrap_err();
    assert!(matches!(err, CoreError::Parse(_)));
}

#[test]
fn snapshot_json_serializes_btree_deterministically() {
    let mut snapshot = BTreeMap::new();
    snapshot.insert("[PRV_EMAIL_2]".to_string(), "b@example.com".to_string());
    snapshot.insert("[PRV_EMAIL_1]".to_string(), "a@example.com".to_string());
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    assert!(json.find("[PRV_EMAIL_1]").unwrap() < json.find("[PRV_EMAIL_2]").unwrap());
}

#[test]
fn mask_with_chars_returns_input_char_count() {
    let masker = PrivacyMasker::new().unwrap();
    let (masked, snapshot, chars) = mask_with_chars(&masker, "mail me at bob@example.com").unwrap();
    assert_eq!(chars, 26);
    assert!(masked.contains("[PRV_EMAIL_"));
    assert!(!masked.contains("bob@example.com"));
    assert!(snapshot.values().any(|value| value == "bob@example.com"));
}

#[test]
fn restore_with_chars_returns_input_char_count() {
    let tmp = common::TempDir::new("restore-chars");
    let snapshot = tmp.path().join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    let (restored, chars) = restore_with_chars("contact [PRV_EMAIL_1]", &snapshot).unwrap();
    assert_eq!(restored, "contact jane@example.com");
    assert_eq!(chars, 21);
}
