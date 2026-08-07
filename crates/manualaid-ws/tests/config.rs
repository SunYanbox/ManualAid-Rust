//! Integration tests for workspace config loading, merging and persistence.
//! 工作区配置加载、合并与持久化的集成测试。

use std::sync::atomic::{AtomicUsize, Ordering};

use manualaid_ws::config::{Config, load, save_project};

/// A unique temporary root directory.
/// 唯一临时根目录。
fn temp_root(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-ws-test-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn load_returns_defaults_when_files_are_missing() {
    let root = temp_root("missing");
    let home = temp_root("missing-home");
    let config = load(&root, &home).unwrap();
    assert_eq!(config, Config::default());
}

#[test]
fn project_config_overrides_global() {
    let root = temp_root("override");
    let home = temp_root("override-home");
    std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
    std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
    std::fs::write(
        home.join(".ManualAid").join("config.toml"),
        "[global]\nlang = \"en\"\ntool_call_format = \"xml\"\n\n[tools]\nshell = true\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".ManualAid").join("config.toml"),
        "[global]\nlang = \"zh-CN\"\n\n[tools]\nshell = false\n",
    )
    .unwrap();
    let config = load(&root, &home).unwrap();
    assert_eq!(config.lang, "zh-CN");
    assert_eq!(config.tool_call_format, "xml");
    assert!(!config.shell);
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn save_project_round_trips_through_load() {
    let root = temp_root("save");
    let home = temp_root("save-home");
    let config = Config {
        lang: "zh-CN".to_string(),
        tool_call_format: "json-codeblock".to_string(),
        shell: false,
        allow_commands: vec!["git status".to_string()],
        ..Config::default()
    };
    save_project(&root, &config).unwrap();
    let loaded = load(&root, &home).unwrap();
    assert_eq!(loaded, config);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn invalid_config_file_is_an_error() {
    let root = temp_root("invalid");
    let home = temp_root("invalid-home");
    std::fs::create_dir_all(root.join(".ManualAid")).unwrap();
    std::fs::write(
        root.join(".ManualAid").join("config.toml"),
        "not [valid toml",
    )
    .unwrap();
    assert!(load(&root, &home).is_err());
    let _ = std::fs::remove_dir_all(&root);
}
