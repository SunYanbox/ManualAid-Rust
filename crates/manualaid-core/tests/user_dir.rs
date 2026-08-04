use manualaid_core::user_dir::UserDirectories;
use manualaid_core::user_dir::{all_directories, cache_dir, config_dir, home_dir};

#[test]
fn test_home_dir_exists() {
    if home_dir().is_err() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
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
fn test_all_directories() {
    if home_dir().is_err() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let dirs = all_directories().expect("all directories should be resolvable");
    assert!(dirs.home.is_absolute());
    assert!(dirs.config.is_absolute());
    assert!(dirs.cache.is_absolute());
    assert!(dirs.data.is_absolute());
}

#[test]
fn test_user_dirs_serialization_roundtrip() {
    if home_dir().is_err() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let dirs = all_directories().unwrap();
    let json = serde_json::to_string(&dirs).unwrap();
    let deserialized: UserDirectories = serde_json::from_str(&json).unwrap();
    assert_eq!(dirs.home, deserialized.home);
    assert_eq!(dirs.config, deserialized.config);
}
