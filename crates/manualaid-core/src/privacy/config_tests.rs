use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique temp directory removed on drop.
/// 移除时清理的唯一临时目录。
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "manualaid-privacy-cfg-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_config(base: &Path, content: &str) {
    let dir = base.join(".ManualAid");
    fs::create_dir_all(&dir).expect("create .ManualAid dir");
    fs::write(dir.join("config.toml"), content).expect("write config");
}

#[test]
fn test_missing_files_yield_empty_extension() {
    let tmp = TempDir::new("missing");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&project).unwrap();
    let ext = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    assert!(ext.regex.is_empty());
    assert!(ext.literal.is_empty());
}

#[test]
fn test_project_overrides_global_per_key() {
    let tmp = TempDir::new("merge");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    write_config(
        &home,
        r#"
[privacy_mask_extension.regex]
A = "^a$"
B = "^b$"

[privacy_mask_extension.literal]
Name = "Alice"
Other = "global"
"#,
    );
    write_config(
        &project,
        r#"
[privacy_mask_extension.regex]
B = "^bb$"
C = "^c$"

[privacy_mask_extension.literal]
Name = "Bob"
"#,
    );

    let ext = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    assert_eq!(ext.regex.get("A").map(String::as_str), Some("^a$"));
    assert_eq!(ext.regex.get("B").map(String::as_str), Some("^bb$"));
    assert_eq!(ext.regex.get("C").map(String::as_str), Some("^c$"));
    assert_eq!(ext.regex.len(), 3);
    assert_eq!(ext.literal.get("Name").map(String::as_str), Some("Bob"));
    assert_eq!(ext.literal.get("Other").map(String::as_str), Some("global"));
    assert_eq!(ext.literal.len(), 2);
}

#[test]
fn test_merge_returns_overridden_full_keys() {
    let global = ExtensionTable {
        regex: HashMap::from([("B".to_string(), "^b$".to_string())]),
        literal: HashMap::from([("Name".to_string(), "Alice".to_string())]),
    };
    let project = ExtensionTable {
        regex: HashMap::from([
            ("B".to_string(), "^bb$".to_string()),
            ("C".to_string(), "^c$".to_string()),
        ]),
        literal: HashMap::from([("Name".to_string(), "Bob".to_string())]),
    };
    let (merged, overridden) = merge_tables(&global, &project);
    assert_eq!(merged.regex.get("B").map(String::as_str), Some("^bb$"));
    assert_eq!(merged.regex.get("C").map(String::as_str), Some("^c$"));
    assert_eq!(merged.literal.get("Name").map(String::as_str), Some("Bob"));
    assert_eq!(
        overridden,
        vec![
            "privacy_mask_extension.regex.B".to_string(),
            "privacy_mask_extension.literal.Name".to_string()
        ]
    );
}

#[test]
fn test_invalid_toml_errors() {
    let tmp = TempDir::new("bad-toml");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    write_config(&home, "this is not toml [[[");
    fs::create_dir_all(&project).unwrap();
    assert!(PrivacyMaskExtension::load_with_home(&project, &home).is_err());
}

#[test]
fn test_empty_project_config_file_is_ignored() {
    let tmp = TempDir::new("empty-project");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    write_config(
        &home,
        "[privacy_mask_extension.literal]\nName = \"Alice\"\n",
    );
    write_config(&project, "[skill]\n");
    let ext = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    assert_eq!(ext.literal.get("Name").map(String::as_str), Some("Alice"));
}

#[test]
fn test_empty_file_yields_empty_extension() {
    let tmp = TempDir::new("empty-file");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    write_config(&home, "");
    fs::create_dir_all(&project).unwrap();
    let ext = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    assert!(ext.regex.is_empty());
    assert!(ext.literal.is_empty());
}

#[test]
fn test_config_path_is_directory_errors() {
    let tmp = TempDir::new("dir-config");
    let home = tmp.0.join("home");
    let project = tmp.0.join("project");
    fs::create_dir_all(home.join(".ManualAid").join("config.toml")).unwrap();
    fs::create_dir_all(&project).unwrap();
    // Reading a directory is an I/O error other than NotFound and must
    // propagate instead of being treated as a missing file.
    // 读取目录属于非 NotFound 的 I/O 错误，必须向上传播而非视为文件缺失。
    assert!(PrivacyMaskExtension::load_with_home(&project, &home).is_err());
}
