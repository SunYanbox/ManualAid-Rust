use std::path::PathBuf;

use manualaid_core::user_dir::UserDirectories;
use manualaid_core::user_dir::{all_directories, cache_dir, config_dir, home_dir};

#[test]
#[ignore = "requires a resolvable real user home; user_dir.rs is coverage-exempt per AGENTS.md"]
fn test_home_dir_exists() {
    let home = home_dir().expect("home dir should be resolvable");
    assert!(home.is_absolute(), "home dir should be absolute: {home:?}");
}

#[test]
fn test_config_dir_exists() {
    let config = config_dir().expect("config dir should be resolvable");
    assert!(
        config.is_absolute(),
        "config dir should be absolute: {config:?}"
    );
}

#[test]
fn test_cache_dir_exists() {
    let cache = cache_dir().expect("cache dir should be resolvable");
    assert!(
        cache.is_absolute(),
        "cache dir should be absolute: {cache:?}"
    );
}

#[test]
#[ignore = "requires a resolvable real user home; user_dir.rs is coverage-exempt per AGENTS.md"]
fn test_all_directories() {
    let dirs = all_directories().expect("all directories should be resolvable");
    assert!(dirs.home.is_absolute());
    assert!(dirs.config.is_absolute());
    assert!(dirs.cache.is_absolute());
    assert!(dirs.data.is_absolute());
}

#[test]
fn test_user_dirs_serialization_roundtrip() {
    // Construct the struct directly so this is a deterministic serde
    // contract test instead of depending on the real home directory.
    // 直接构造结构体，使本测试成为确定性的 serde 契约测试，
    // 而不依赖真实 home 目录。
    let dirs = UserDirectories {
        home: PathBuf::from("C:/Users/alice"),
        config: PathBuf::from("C:/Users/alice/AppData/Roaming"),
        cache: PathBuf::from("C:/Users/alice/AppData/Local"),
        data: PathBuf::from("C:/Users/alice/AppData/Roaming"),
    };
    let json = serde_json::to_string(&dirs).unwrap();
    let deserialized: UserDirectories = serde_json::from_str(&json).unwrap();
    assert_eq!(dirs.home, deserialized.home);
    assert_eq!(dirs.config, deserialized.config);
    assert_eq!(dirs.cache, deserialized.cache);
    assert_eq!(dirs.data, deserialized.data);
}
