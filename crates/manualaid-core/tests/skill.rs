// Tests serialize against the shared skill store static with a std Mutex;
// the guard is never held across awaits, so the lint does not apply.
// 测试用 std Mutex 串行化对共享技能存储静态变量的访问；守卫不会跨 await
// 持有，此 lint 不适用。
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use manualaid_core::error::CoreError;
use manualaid_core::skill::{
    all_skills, enabled_skills, get_skill, reload_skills, reload_skills_with_home, reset_skills,
    set_enabled,
};
use manualaid_core::user_dir;

mod common;
use common::{TempDir, write_skill};

/// Serializes tests that touch the shared skill store static, because test
/// bodies run concurrently.
/// 串行化触及共享技能存储静态变量的测试（测试体并发运行）。
static SKILL_LOCK: Mutex<()> = Mutex::new(());

/// Resets the skill store on drop. Declared after the lock guard so the
/// reset happens before the lock is released. The guard field is only held
/// for its `Drop` behavior, never read.
/// 析构时重置技能存储。在锁守卫之后声明，确保先重置后解锁。守卫字段仅为
/// 其 Drop 行为而持有，从不读取。
#[allow(dead_code)]
struct SkillsRestore(MutexGuard<'static, ()>);

impl Drop for SkillsRestore {
    fn drop(&mut self) {
        reset_skills();
    }
}

fn lock_skills() -> SkillsRestore {
    SkillsRestore(SKILL_LOCK.lock().unwrap_or_else(PoisonError::into_inner))
}

/// `reload_skills` loads a project skill with all fields populated.
/// `reload_skills` 加载项目技能，所有字段均已填充。
#[test]
fn reload_skills_loads_project_skill() {
    let _restore = lock_skills();
    let root = TempDir::new("project-skill");
    let home = TempDir::new("project-skill-home");
    write_skill(
        &root.path().join(".claude"),
        "greeter",
        Some("greeter"),
        "A greeting skill",
        "## Usage\nHello",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    let skill = &skills[0];
    assert_eq!(skill.unique_name, "greeter");
    assert_eq!(skill.name, "greeter");
    assert_eq!(skill.description, "A greeting skill");
    assert_eq!(skill.body, "## Usage\nHello");
    assert_eq!(
        skill.path,
        root.path().join(".claude").join("skills").join("greeter")
    );
    assert!(!skill.is_global);
    assert!(skill.is_enabled);
}

/// `reload_skills` loads a global skill disabled by default.
/// `reload_skills` 加载全局技能，默认禁用。
#[test]
fn reload_skills_loads_global_skill_with_default_disabled() {
    let _restore = lock_skills();
    let root = TempDir::new("global-skill-root");
    let home = TempDir::new("global-skill-home");
    write_skill(
        &home.path().join(".codex"),
        "helper",
        Some("helper"),
        "A global skill",
        "body",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    let skill = &skills[0];
    assert_eq!(skill.name, "helper");
    assert!(skill.is_global);
    assert!(!skill.is_enabled);
}

/// `reload_skills` returns an empty store for missing search directories.
/// 搜索目录缺失时 `reload_skills` 返回空存储。
#[test]
fn reload_skills_missing_dirs_is_empty_and_ok() {
    let _restore = lock_skills();
    let root = TempDir::new("empty-root");
    let home = TempDir::new("empty-home");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(all_skills().is_empty());
}

/// Skills without a `SKILL.md` file or non-directory entries are skipped.
/// 没有 `SKILL.md` 文件的文件夹与非目录条目被跳过。
#[test]
fn reload_skills_skips_folder_without_skill_md_and_non_dirs() {
    let _restore = lock_skills();
    let root = TempDir::new("skip-invalid");
    let home = TempDir::new("skip-invalid-home");
    write_skill(
        &root.path().join(".claude"),
        "valid",
        Some("valid"),
        "ok",
        "body",
    );
    std::fs::create_dir_all(root.path().join(".claude").join("skills").join("no-md"))
        .expect("create folder");
    std::fs::write(
        root.path().join(".claude").join("skills").join("file.txt"),
        "x",
    )
    .expect("write file");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "valid");
}

/// Skills with an empty or missing `description:` are skipped.
/// `description:` 为空或缺失的技能被跳过。
#[test]
fn reload_skills_skips_empty_description() {
    let _restore = lock_skills();
    let root = TempDir::new("skip-empty-desc");
    let home = TempDir::new("skip-empty-desc-home");
    write_skill(
        &root.path().join(".claude"),
        "empty-desc",
        Some("empty-desc"),
        "",
        "body",
    );
    write_skill(
        &root.path().join(".claude"),
        "no-desc",
        Some("no-desc"),
        "dummy",
        "body",
    );
    // Overwrite the second skill's frontmatter to drop the description line.
    std::fs::write(
        root.path()
            .join(".claude")
            .join("skills")
            .join("no-desc")
            .join("SKILL.md"),
        "---\nname: no-desc\n---\nbody",
    )
    .expect("write SKILL.md");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(all_skills().is_empty());
}

/// A `SKILL.md` that is a directory or has no frontmatter is skipped.
/// `SKILL.md` 为目录或没有 frontmatter 的技能被跳过。
#[test]
fn reload_skills_skips_malformed_skill_md() {
    let _restore = lock_skills();
    let root = TempDir::new("malformed");
    let home = TempDir::new("malformed-home");
    std::fs::create_dir_all(
        root.path()
            .join(".claude")
            .join("skills")
            .join("bad-dir")
            .join("SKILL.md"),
    )
    .expect("create SKILL.md as directory");
    std::fs::create_dir_all(root.path().join(".claude").join("skills").join("no-fm"))
        .expect("create folder");
    std::fs::write(
        root.path()
            .join(".claude")
            .join("skills")
            .join("no-fm")
            .join("SKILL.md"),
        "no frontmatter here",
    )
    .expect("write SKILL.md");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(all_skills().is_empty());
}

/// A missing `name:` falls back to the folder name.
/// `name:` 缺失时回退为文件夹名。
#[test]
fn reload_skills_name_falls_back_to_folder() {
    let _restore = lock_skills();
    let root = TempDir::new("name-fallback");
    let home = TempDir::new("name-fallback-home");
    write_skill(
        &root.path().join(".claude"),
        "folder-name",
        None,
        "desc",
        "body",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "folder-name");
    assert_eq!(skills[0].unique_name, "folder-name");
}

/// Identical skills in project and global scope keep the project copy.
/// 项目与全局范围内容相同的技能保留项目副本。
#[test]
fn reload_skills_dedup_prefers_project() {
    let _restore = lock_skills();
    let root = TempDir::new("dedup-root");
    let home = TempDir::new("dedup-home");
    write_skill(
        &root.path().join(".claude"),
        "dup",
        Some("dup"),
        "same",
        "same body",
    );
    write_skill(
        &home.path().join(".codex"),
        "dup",
        Some("dup"),
        "same",
        "same body",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    assert!(!skills[0].is_global);
    assert_eq!(
        skills[0].path,
        root.path().join(".claude").join("skills").join("dup")
    );
}

/// Skills found in several agent directories of the same scope are all
/// scanned; identical ones keep the first (`.claude` precedes `.codex`).
/// 同一范围内多个 agent 目录中的技能都会被扫描；内容相同的保留先出现的
/// （`.claude` 先于 `.codex`）。
#[test]
fn reload_skills_searches_all_agent_dirs() {
    let _restore = lock_skills();
    let root = TempDir::new("agent-dirs");
    let home = TempDir::new("agent-dirs-home");
    write_skill(
        &root.path().join(".claude"),
        "a",
        Some("a"),
        "from claude",
        "body",
    );
    write_skill(
        &root.path().join(".codex"),
        "b",
        Some("b"),
        "from codex",
        "body",
    );
    write_skill(
        &root.path().join(".claude"),
        "same",
        Some("same"),
        "same",
        "same",
    );
    write_skill(
        &root.path().join(".codex"),
        "same",
        Some("same"),
        "same",
        "same",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 3);
    // The dedup preference (shorter path wins on length ties) is covered by
    // unit tests; here only the deduplicated result is asserted.
    assert!(
        get_skill("same")
            .expect("skill should exist")
            .path
            .is_absolute()
    );
    assert!(get_skill("a").is_some());
    assert!(get_skill("b").is_some());
}

/// A name collision with different content keeps both, renaming the global
/// copy with a scope prefix.
/// 名称相同但内容不同时两者都保留，全局副本以作用域前缀重命名。
#[test]
fn reload_skills_renames_global_name_conflict() {
    let _restore = lock_skills();
    let root = TempDir::new("rename-root");
    let home = TempDir::new("rename-home");
    write_skill(
        &root.path().join(".claude"),
        "a",
        Some("a"),
        "project desc",
        "p",
    );
    write_skill(
        &home.path().join(".codex"),
        "a",
        Some("a"),
        "global desc",
        "g",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 2);
    let project = get_skill("a").expect("project copy should keep the plain name");
    let global = get_skill(".global-a").expect("global copy should be renamed");
    assert_eq!(project.description, "project desc");
    assert_eq!(global.description, "global desc");
    assert!(global.is_global);
    assert!(!global.is_enabled);
}

/// Config entries override the default enable states.
/// 配置条目覆盖默认启用状态。
#[test]
fn reload_skills_applies_config_overrides() {
    let _restore = lock_skills();
    let root = TempDir::new("config-override-root");
    let home = TempDir::new("config-override-home");
    let project_skill = write_skill(&root.path().join(".claude"), "p", Some("p"), "proj", "body");
    let global_skill = write_skill(&home.path().join(".agents"), "g", Some("g"), "glob", "body");

    let config_dir = root.path().join(".ManualAid");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    let key_p = project_skill.to_string_lossy().replace('\\', "/");
    let key_g = global_skill.to_string_lossy().replace('\\', "/");
    std::fs::write(
        config_dir.join("config.toml"),
        format!("[skill]\n\"{key_p}\" = false\n\"{key_g}\" = true\n"),
    )
    .expect("write config");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(!get_skill("p").expect("project skill").is_enabled);
    assert!(get_skill("g").expect("global skill").is_enabled);
}

/// Without a config file the defaults apply: project enabled, global
/// disabled.
/// 无配置文件时应用默认值：项目启用、全局禁用。
#[test]
fn reload_skills_missing_config_uses_defaults() {
    let _restore = lock_skills();
    let root = TempDir::new("no-config-root");
    let home = TempDir::new("no-config-home");
    write_skill(&root.path().join(".claude"), "p", Some("p"), "proj", "body");
    write_skill(&home.path().join(".codex"), "g", Some("g"), "glob", "body");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(get_skill("p").expect("project skill").is_enabled);
    assert!(!get_skill("g").expect("global skill").is_enabled);
}

/// A reload replaces the previous store entirely.
/// 重新加载会整体替换原有存储。
#[test]
fn reload_skills_replaces_previous_store() {
    let _restore = lock_skills();
    let root = TempDir::new("replace-store");
    let home = TempDir::new("replace-store-home");
    write_skill(
        &root.path().join(".claude"),
        "first",
        Some("first"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("first reload should succeed");
    assert_eq!(all_skills().len(), 1);

    std::fs::remove_dir_all(root.path().join(".claude").join("skills").join("first"))
        .expect("remove first skill");
    write_skill(
        &root.path().join(".claude"),
        "second",
        Some("second"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("second reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "second");
}

/// An invalid config file fails the reload and leaves the old store intact.
/// 无效的配置文件使 reload 失败，原有存储保持不变。
#[test]
fn reload_skills_invalid_config_is_config_error() {
    let _restore = lock_skills();
    let root = TempDir::new("invalid-config");
    let home = TempDir::new("invalid-config-home");
    write_skill(
        &root.path().join(".claude"),
        "good",
        Some("good"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    std::fs::create_dir_all(root.path().join(".ManualAid")).expect("create config dir");
    std::fs::write(
        root.path().join(".ManualAid").join("config.toml"),
        "not = [valid toml",
    )
    .expect("write config");

    let err = reload_skills_with_home(root.path(), home.path()).expect_err("reload should fail");
    assert!(matches!(err, CoreError::Config(_)));
    assert_eq!(all_skills().len(), 1);
    assert_eq!(all_skills()[0].name, "good");
}

/// The public `reload_skills` works against the real user home; the project
/// skill is loaded regardless of what skills the real home holds.
/// 公开的 `reload_skills` 针对真实用户主目录可用；无论真实主目录中有哪些
/// 技能，项目技能都会被加载。
#[test]
fn reload_skills_uses_real_home() {
    if user_dir::home_dir().is_err() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let _restore = lock_skills();
    let root = TempDir::new("real-home-skills");
    write_skill(
        &root.path().join(".claude"),
        "mine",
        Some("mine"),
        "desc",
        "body",
    );

    reload_skills(root.path()).expect("reload should succeed");
    assert!(get_skill("mine").is_some());
}

/// `get_skill` returns the skill by unique name or `None`.
/// `get_skill` 按唯一名称返回技能，未找到时返回 `None`。
#[test]
fn get_skill_returns_matching_skill() {
    let _restore = lock_skills();
    let root = TempDir::new("get-skill");
    let home = TempDir::new("get-skill-home");
    write_skill(
        &root.path().join(".claude"),
        "found",
        Some("found"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    assert_eq!(get_skill("found").expect("skill").name, "found");
    assert!(get_skill("missing").is_none());
}

/// `enabled_skills` returns only enabled skills.
/// `enabled_skills` 只返回已启用的技能。
#[test]
fn enabled_skills_returns_only_enabled() {
    let _restore = lock_skills();
    let root = TempDir::new("enabled-root");
    let home = TempDir::new("enabled-home");
    let project = write_skill(&root.path().join(".claude"), "p", Some("p"), "proj", "body");
    write_skill(&home.path().join(".codex"), "g", Some("g"), "glob", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    assert_eq!(enabled_skills().len(), 1);
    assert_eq!(enabled_skills()[0].name, "p");

    set_enabled(&project, false).expect("disable should succeed");
    assert!(enabled_skills().is_empty());
}

/// `set_enabled` updates the store and persists to the config file.
/// `set_enabled` 更新存储并持久化到配置文件。
#[test]
fn set_enabled_updates_store_and_roundtrips() {
    let _restore = lock_skills();
    let root = TempDir::new("set-enabled");
    let home = TempDir::new("set-enabled-home");
    let skill = write_skill(&root.path().join(".claude"), "s", Some("s"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    set_enabled(&skill, false).expect("disable should succeed");
    assert!(!get_skill("s").expect("skill").is_enabled);

    let content = std::fs::read_to_string(root.path().join(".ManualAid").join("config.toml"))
        .expect("config should exist");
    let table: toml::Table = toml::from_str(&content).expect("parse config");
    let key = skill.to_string_lossy().replace('\\', "/");
    assert_eq!(table["skill"][&key], toml::Value::Boolean(false));

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(!get_skill("s").expect("skill").is_enabled);
}

/// Concurrent `set_enabled` calls do not lose each other's persisted entries.
/// 并发的 `set_enabled` 调用不会互相丢失已持久化的条目。
#[test]
fn set_enabled_concurrent_calls_do_not_lose_updates() {
    let _restore = lock_skills();
    let root = TempDir::new("concurrent-set-enabled");
    let home = TempDir::new("concurrent-set-enabled-home");
    let project = root.path().join(".claude");
    std::fs::create_dir_all(&project).expect("create project agent dir");
    let mut paths = Vec::new();
    for i in 0..8 {
        let path = write_skill(&project, &format!("skill{i}"), None, "desc", "body");
        paths.push(path);
    }
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    let handles: Vec<_> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let path = path.clone();
            std::thread::spawn(move || {
                set_enabled(&path, i % 2 == 0).expect("set_enabled should succeed");
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread should finish");
    }

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let all = all_skills();
    for (i, path) in paths.iter().enumerate() {
        let skill = all
            .iter()
            .find(|skill| &skill.path == path)
            .expect("skill should be loaded");
        assert_eq!(
            skill.is_enabled,
            i % 2 == 0,
            "lost update for {}",
            path.display()
        );
    }
}

/// `set_enabled` creates the config file and its parent directory when
/// missing, and preserves existing entries across calls.
/// 配置文件缺失时 `set_enabled` 创建文件与其父目录，多次调用保留已有条目。
#[test]
fn set_enabled_creates_config_when_missing_and_merges() {
    let _restore = lock_skills();
    let root = TempDir::new("create-config");
    let home = TempDir::new("create-config-home");
    let first = write_skill(
        &root.path().join(".claude"),
        "first",
        Some("first"),
        "desc",
        "body",
    );
    let second = write_skill(
        &root.path().join(".claude"),
        "second",
        Some("second"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    set_enabled(&first, false).expect("disable should succeed");
    assert!(root.path().join(".ManualAid").join("config.toml").is_file());

    set_enabled(&second, false).expect("disable should succeed");
    let content = std::fs::read_to_string(root.path().join(".ManualAid").join("config.toml"))
        .expect("read config");
    let table: toml::Table = toml::from_str(&content).expect("parse config");
    let skill = &table["skill"];
    let key_first = first.to_string_lossy().replace('\\', "/");
    let key_second = second.to_string_lossy().replace('\\', "/");
    assert_eq!(skill[&key_first], toml::Value::Boolean(false));
    assert_eq!(skill[&key_second], toml::Value::Boolean(false));
}

/// `set_enabled` preserves hand-written sections of the config file.
/// `set_enabled` 保留配置文件中用户手写的其他配置节。
#[test]
fn set_enabled_preserves_other_sections() {
    let _restore = lock_skills();
    let root = TempDir::new("preserve-sections");
    let home = TempDir::new("preserve-sections-home");
    let skill = write_skill(&root.path().join(".claude"), "s", Some("s"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    let config_dir = root.path().join(".ManualAid");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    std::fs::write(config_dir.join("config.toml"), "[other]\nfoo = 1\n").expect("write config");

    set_enabled(&skill, false).expect("disable should succeed");
    let content = std::fs::read_to_string(config_dir.join("config.toml")).expect("read config");
    let table: toml::Table = toml::from_str(&content).expect("parse config");
    assert_eq!(table["other"]["foo"], toml::Value::Integer(1));
    assert!(table.contains_key("skill"));
}

/// `set_enabled` with an unknown path errors without persisting.
/// 未知路径的 `set_enabled` 报错且不写入。
#[test]
fn set_enabled_unknown_path_errors_without_persisting() {
    let _restore = lock_skills();
    let root = TempDir::new("unknown-path");
    let home = TempDir::new("unknown-path-home");
    write_skill(&root.path().join(".claude"), "s", Some("s"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    let err = set_enabled(&root.path().join("nope"), true).expect_err("should fail");
    assert!(matches!(err, CoreError::NotFound(_)));
    assert!(!root.path().join(".ManualAid").join("config.toml").exists());
}

/// `set_enabled` before any reload errors.
/// 未 reload 时调用 `set_enabled` 报错。
#[test]
fn set_enabled_before_reload_errors() {
    let _restore = lock_skills();
    reset_skills();
    let root = TempDir::new("before-reload");
    let err = set_enabled(&root.path().join("x"), true).expect_err("should fail");
    assert!(matches!(err, CoreError::NotFound(_)));
}

/// `set_enabled` enables a global skill and persists the state.
/// `set_enabled` 启用全局技能并持久化状态。
#[test]
fn set_enabled_enables_global_skill() {
    let _restore = lock_skills();
    let root = TempDir::new("enable-global-root");
    let home = TempDir::new("enable-global-home");
    let skill = write_skill(&home.path().join(".codex"), "g", Some("g"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    set_enabled(&skill, true).expect("enable should succeed");
    assert!(get_skill("g").expect("skill").is_enabled);

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(get_skill("g").expect("skill").is_enabled);
}

/// A failed config write leaves the store unchanged.
/// 配置写入失败时存储保持不变。
#[test]
fn set_enabled_persist_failure_leaves_store_unchanged() {
    let _restore = lock_skills();
    let root = TempDir::new("persist-fail");
    let home = TempDir::new("persist-fail-home");
    let skill = write_skill(&root.path().join(".claude"), "s", Some("s"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    std::fs::write(root.path().join(".ManualAid"), "file").expect("block config dir");

    let err = set_enabled(&skill, false).expect_err("should fail");
    assert!(matches!(err, CoreError::Io(_)));
    assert!(get_skill("s").expect("skill").is_enabled);
}

/// `set_enabled` accepts a relative path and resolves it against the
/// current directory.
/// `set_enabled` 接受相对路径并相对当前目录解析。
#[test]
fn set_enabled_accepts_relative_path() {
    let _restore = lock_skills();
    let root = TempDir::new("relative-path");
    let home = TempDir::new("relative-path-home");
    write_skill(
        &root.path().join(".claude"),
        "rel",
        Some("rel"),
        "desc",
        "body",
    );
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    let cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(root.path()).expect("change dir");
    let result = set_enabled(Path::new(".claude/skills/rel"), false);
    std::env::set_current_dir(&cwd).expect("restore dir");
    result.expect("relative path should resolve");

    assert!(!get_skill("rel").expect("skill").is_enabled);
}
