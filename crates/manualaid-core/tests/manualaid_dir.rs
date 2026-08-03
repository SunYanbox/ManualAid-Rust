use manualaid_core::error::CoreError;
use manualaid_core::manualaid_dir::{
    DEFAULT_GLOBAL_CONFIG_CONTENT, DEFAULT_PROJECT_CONFIG_CONTENT, GITIGNORE_CONTENT,
    ensure_manualaid_dirs, ensure_manualaid_dirs_with_home,
};

mod common;
use common::TempDir;

/// `ensure_manualaid_dirs` creates the home `.ManualAid` dir and its config
/// file with the default content.
/// `ensure_manualaid_dirs` 创建主目录下的 `.ManualAid` 目录与默认内容的
/// 配置文件。
#[test]
fn ensure_creates_home_dir_and_config() {
    let root = TempDir::new("home-config-root");
    let home = TempDir::new("home-config-home");

    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("ensure should succeed");

    let config = home.path().join(".ManualAid").join("config.toml");
    assert!(config.is_file());
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        DEFAULT_GLOBAL_CONFIG_CONTENT
    );
}

/// `ensure_manualaid_dirs` creates the project `.ManualAid` dir with a
/// byte-exact `.gitignore` and a config file with the default `[skill]`
/// content.
/// `ensure_manualaid_dirs` 创建项目下的 `.ManualAid` 目录、字节精确的
/// `.gitignore` 与默认 `[skill]` 内容的配置文件。
#[test]
fn ensure_creates_project_dir_and_files() {
    let root = TempDir::new("project-files");
    let home = TempDir::new("project-files-home");

    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("ensure should succeed");

    let gitignore = root.path().join(".ManualAid").join(".gitignore");
    assert!(gitignore.is_file());
    assert_eq!(
        std::fs::read_to_string(&gitignore).expect("read gitignore"),
        GITIGNORE_CONTENT
    );
    let config = root.path().join(".ManualAid").join("config.toml");
    assert!(config.is_file());
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        DEFAULT_PROJECT_CONFIG_CONTENT
    );
}

/// Running `ensure_manualaid_dirs` twice succeeds both times.
/// 连续两次运行 `ensure_manualaid_dirs` 均成功。
#[test]
fn ensure_is_idempotent() {
    let root = TempDir::new("idempotent");
    let home = TempDir::new("idempotent-home");

    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("first ensure");
    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("second ensure");

    assert!(root.path().join(".ManualAid").join(".gitignore").is_file());
    assert!(root.path().join(".ManualAid").join("config.toml").is_file());
    assert!(home.path().join(".ManualAid").join("config.toml").is_file());
}

/// Existing files are never overwritten by `ensure_manualaid_dirs`.
/// 已存在的文件不会被 `ensure_manualaid_dirs` 覆盖。
#[test]
fn ensure_never_overwrites_existing_files() {
    let root = TempDir::new("no-overwrite");
    let home = TempDir::new("no-overwrite-home");

    std::fs::create_dir_all(root.path().join(".ManualAid")).expect("create dir");
    std::fs::write(root.path().join(".ManualAid").join(".gitignore"), "custom")
        .expect("write gitignore");
    std::fs::write(
        root.path().join(".ManualAid").join("config.toml"),
        "[custom]",
    )
    .expect("write config");
    std::fs::create_dir_all(home.path().join(".ManualAid")).expect("create dir");
    std::fs::write(
        home.path().join(".ManualAid").join("config.toml"),
        "[custom]",
    )
    .expect("write config");

    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("ensure should succeed");

    assert_eq!(
        std::fs::read_to_string(root.path().join(".ManualAid").join(".gitignore"))
            .expect("read gitignore"),
        "custom"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join(".ManualAid").join("config.toml"))
            .expect("read config"),
        "[custom]"
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join(".ManualAid").join("config.toml"))
            .expect("read config"),
        "[custom]"
    );
}

/// `ensure_manualaid_dirs` errors when the home path is a file.
/// home 路径为文件时 `ensure_manualaid_dirs` 报错。
#[test]
fn ensure_fails_when_home_path_is_a_file() {
    let root = TempDir::new("home-file-root");
    let home_file = TempDir::new("home-file-home");
    let blocker = home_file.path().join("blocker");
    std::fs::write(&blocker, "file").expect("write blocker");
    let home_blocker = blocker.join("nested");

    let err = ensure_manualaid_dirs_with_home(root.path(), &home_blocker)
        .expect_err("ensure should fail");
    assert!(matches!(err, CoreError::Io(_)));
}

/// The public `ensure_manualaid_dirs` works against the real user home.
/// It is non-destructive: files in the real home are only created when
/// missing, never overwritten.
/// 公开的 `ensure_manualaid_dirs` 针对真实用户主目录可用。该测试是
/// 非破坏性的：真实主目录中的文件仅在缺失时创建，绝不覆盖。
#[test]
fn ensure_works_with_real_home() {
    let root = TempDir::new("real-home");
    ensure_manualaid_dirs(root.path()).expect("ensure should succeed");
    assert!(root.path().join(".ManualAid").join(".gitignore").is_file());
    assert!(root.path().join(".ManualAid").join("config.toml").is_file());
}
