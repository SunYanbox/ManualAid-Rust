use manualaid_core::error::CoreError;
use manualaid_core::manualaid_dir::{
    DEFAULT_GLOBAL_CONFIG_CONTENT, DEFAULT_PROJECT_CONFIG_CONTENT, GITIGNORE_CONTENT,
    clean_manualaid_dir, ensure_global_manualaid_dir, ensure_manualaid_dirs,
    ensure_manualaid_dirs_with_home, ensure_project_manualaid_dir,
};
use manualaid_core::privacy::PrivacyMaskExtension;
use manualaid_core::user_dir;

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
/// byte-exact `.gitignore` and a config file with the default commented
/// content.
/// `ensure_manualaid_dirs` 创建项目下的 `.ManualAid` 目录、字节精确的
/// `.gitignore` 与带默认注释内容的配置文件。
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

/// The default generated config files parse as privacy mask extensions with
/// empty tables.
/// 默认生成的配置文件可被隐私掩码扩展加载器解析，两个表均为空。
#[test]
fn ensure_default_config_parses_as_privacy_extension() {
    let root = TempDir::new("default-config-privacy");
    let home = TempDir::new("default-config-privacy-home");

    ensure_manualaid_dirs_with_home(root.path(), home.path()).expect("ensure should succeed");

    let ext = PrivacyMaskExtension::load_with_home(root.path(), home.path())
        .expect("default config should parse");
    assert!(ext.regex.is_empty());
    assert!(ext.literal.is_empty());
}

/// Both default templates cover every section the runtime actually loads,
/// with example keys commented out, and parse as valid TOML.
/// 两个默认模板都覆盖运行时会加载的所有配置节（示例键均注释），并解析
/// 为合法 TOML。
#[test]
fn default_templates_cover_all_loaded_sections() {
    let global: toml::Table =
        toml::from_str(DEFAULT_GLOBAL_CONFIG_CONTENT).expect("global template parses");
    let project: toml::Table =
        toml::from_str(DEFAULT_PROJECT_CONFIG_CONTENT).expect("project template parses");

    for table in [&global, &project] {
        for section in ["global", "tools", "permissions", "privacy_mask_extension"] {
            assert!(table.contains_key(section), "missing `{section}` section");
        }
        let privacy = &table["privacy_mask_extension"];
        assert!(privacy.get("regex").is_some(), "missing `regex` table");
        assert!(privacy.get("literal").is_some(), "missing `literal` table");
    }

    // `[skill]` is project-only: the global file never carries skill state.
    // `[skill]` 仅存在于项目配置：全局文件不承载技能启用状态。
    assert!(!global.contains_key("skill"));
    assert!(project.contains_key("skill"));
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
    if user_dir::home_dir().is_err() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let root = TempDir::new("real-home");
    ensure_manualaid_dirs(root.path()).expect("ensure should succeed");
    assert!(root.path().join(".ManualAid").join(".gitignore").is_file());
    assert!(root.path().join(".ManualAid").join("config.toml").is_file());
}

/// `ensure_project_manualaid_dir` creates only the project `.ManualAid`
/// files and never touches the home directory.
/// `ensure_project_manualaid_dir` 只创建项目 `.ManualAid` 文件，不触碰主目录。
#[test]
fn ensure_project_creates_only_project_files() {
    let root = TempDir::new("project-only");
    let home = TempDir::new("project-only-home");

    ensure_project_manualaid_dir(root.path()).expect("ensure project should succeed");

    assert!(root.path().join(".ManualAid").join("config.toml").is_file());
    assert!(root.path().join(".ManualAid").join(".gitignore").is_file());
    assert!(!home.path().join(".ManualAid").exists());
}

/// `ensure_global_manualaid_dir` creates only the global `.ManualAid` config
/// and never touches the project directory.
/// `ensure_global_manualaid_dir` 只创建全局 `.ManualAid` 配置，不触碰项目目录。
#[test]
fn ensure_global_creates_only_global_file() {
    let root = TempDir::new("global-only");
    let home = TempDir::new("global-only-home");

    ensure_global_manualaid_dir(home.path()).expect("ensure global should succeed");

    let config = home.path().join(".ManualAid").join("config.toml");
    assert!(config.is_file());
    assert_eq!(
        std::fs::read_to_string(&config).expect("read config"),
        DEFAULT_GLOBAL_CONFIG_CONTENT
    );
    assert!(!root.path().join(".ManualAid").exists());
}

/// Both scope-specific `ensure_*` functions are idempotent.
/// 两个按作用域拆分的 `ensure_*` 函数均幂等。
#[test]
fn ensure_scope_functions_are_idempotent() {
    let root = TempDir::new("scope-idempotent");
    let home = TempDir::new("scope-idempotent-home");

    ensure_project_manualaid_dir(root.path()).expect("first project ensure");
    ensure_project_manualaid_dir(root.path()).expect("second project ensure");
    ensure_global_manualaid_dir(home.path()).expect("first global ensure");
    ensure_global_manualaid_dir(home.path()).expect("second global ensure");

    assert!(root.path().join(".ManualAid").join("config.toml").is_file());
    assert!(home.path().join(".ManualAid").join("config.toml").is_file());
}

/// `clean_manualaid_dir` removes the whole `.ManualAid` directory, keeps the
/// base directory, and reports the exact file count and byte sum.
/// `clean_manualaid_dir` 删除整个 `.ManualAid` 目录、保留 base 目录，并返回
/// 精确的文件数与字节总和。
#[test]
fn clean_removes_dir_and_reports_stats() {
    let base = TempDir::new("clean-stats");
    let manual = base.path().join(".ManualAid");
    std::fs::create_dir_all(manual.join("logs")).expect("create logs");
    std::fs::write(manual.join("config.toml"), "[skill]").expect("write config");
    std::fs::write(manual.join("data.bin"), vec![0u8; 1024]).expect("write data");
    std::fs::write(manual.join("logs").join("a.log"), "x").expect("write log");

    let report = clean_manualaid_dir(base.path())
        .expect("clean should succeed")
        .expect("report should be present");

    assert_eq!(report.files, 3);
    assert_eq!(report.bytes, 1024 + 7 + 1);
    assert!(!manual.exists());
    assert!(base.path().exists());
}

/// `clean_manualaid_dir` returns `None` when `.ManualAid` does not exist.
/// `.ManualAid` 不存在时 `clean_manualaid_dir` 返回 `None`。
#[test]
fn clean_missing_returns_none() {
    let base = TempDir::new("clean-missing");
    assert!(
        clean_manualaid_dir(base.path())
            .expect("clean should succeed")
            .is_none()
    );
}

/// `clean_manualaid_dir` errors when `.ManualAid` exists but is a file.
/// `.ManualAid` 存在但不是目录时 `clean_manualaid_dir` 报错。
#[test]
fn clean_file_path_is_invalid() {
    let base = TempDir::new("clean-file");
    std::fs::write(base.path().join(".ManualAid"), "file").expect("write blocker");

    let err = clean_manualaid_dir(base.path()).expect_err("clean should fail");
    assert!(matches!(err, CoreError::InvalidPath(_)));
    assert!(base.path().join(".ManualAid").exists());
}
