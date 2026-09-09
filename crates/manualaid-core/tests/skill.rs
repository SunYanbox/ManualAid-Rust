// Tests serialize against the shared skill store static with a std Mutex;
// the guard is never held across awaits, so the lint does not apply.
// 测试用 std Mutex 串行化对共享技能存储静态变量的访问；守卫不会跨 await
// 持有，此 lint 不适用。
#![allow(clippy::await_holding_lock)]

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use manualaid_core::error::CoreError;
use manualaid_core::manualaid_dir::DEFAULT_PROJECT_CONFIG_CONTENT;
use manualaid_core::skill::{
    all_skills, enabled_skills, exposed_name_map, get_skill, reload_skills,
    reload_skills_with_home, reset_skills, resolve_skill, set_enabled,
};

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
    assert_eq!(skill.unique_name, "project-.claude-greeter");
    assert_eq!(skill.name, "greeter");
    assert_eq!(skill.agent_dir, ".claude");
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
    assert_eq!(skill.unique_name, "global-.codex-helper");
    assert_eq!(skill.name, "helper");
    assert_eq!(skill.agent_dir, ".codex");
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
    assert_eq!(skills[0].unique_name, "project-.claude-folder-name");
    assert_eq!(skills[0].agent_dir, ".claude");
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
        resolve_skill("same")
            .expect("skill should exist")
            .path
            .is_absolute()
    );
    assert!(resolve_skill("a").is_some());
    assert!(resolve_skill("b").is_some());
}

/// A plain name shared across scopes keeps both copies with stable names;
/// the exposed name changes as the enabled set changes.
/// 跨作用域共享裸名时两个副本均以稳定名保留；暴露名随启用集合变化。
#[test]
fn reload_skills_keeps_both_copies_with_stable_names() {
    let _restore = lock_skills();
    let root = TempDir::new("stable-names-root");
    let home = TempDir::new("stable-names-home");
    write_skill(
        &root.path().join(".claude"),
        "a",
        Some("a"),
        "project desc",
        "p",
    );
    let global_path = write_skill(
        &home.path().join(".codex"),
        "a",
        Some("a"),
        "global desc",
        "g",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 2);
    let project = get_skill("project-.claude-a").expect("stable name should match");
    let global = get_skill("global-.codex-a").expect("stable name should match");
    assert_eq!(project.description, "project desc");
    assert_eq!(global.description, "global desc");
    assert!(global.is_global);
    assert!(!global.is_enabled);
    // Only the project copy is enabled, so its plain name resolves.
    // 只有项目副本启用，其裸名可解析。
    assert_eq!(
        resolve_skill("a").expect("enabled copy").description,
        "project desc"
    );

    set_enabled(&global_path, true).expect("enable global copy");
    // Both copies enabled: the plain name is ambiguous, the dir-prefixed
    // exposed names resolve to their copies.
    // 两个副本都启用：裸名有歧义，目录前缀暴露名可解析到对应副本。
    assert!(resolve_skill("a").is_none());
    let hit = resolve_skill(".claude-a").expect("dir-prefixed name");
    assert_eq!(hit.description, "project desc");
    let hit = resolve_skill(".codex-a").expect("dir-prefixed name");
    assert_eq!(hit.description, "global desc");
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
    assert!(
        !get_skill("project-.claude-p")
            .expect("project skill")
            .is_enabled
    );
    assert!(
        get_skill("global-.agents-g")
            .expect("global skill")
            .is_enabled
    );
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
    assert!(
        get_skill("project-.claude-p")
            .expect("project skill")
            .is_enabled
    );
    assert!(
        !get_skill("global-.codex-g")
            .expect("global skill")
            .is_enabled
    );
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
#[ignore = "requires a resolvable real user home; deterministic coverage is reload_skills_with_home"]
fn reload_skills_uses_real_home() {
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
    assert!(get_skill("project-.claude-mine").is_some());
}

/// `get_skill` matches the full stable unique name only; `resolve_skill`
/// handles agent-facing names.
/// `get_skill` 仅按完整稳定唯一名称匹配；`resolve_skill` 处理代理可见名称。
#[test]
fn get_skill_matches_stable_name_only() {
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

    assert_eq!(
        get_skill("project-.claude-found")
            .expect("stable name")
            .name,
        "found"
    );
    assert!(get_skill("found").is_none());
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
    assert!(!get_skill("project-.claude-s").expect("skill").is_enabled);

    let content = std::fs::read_to_string(root.path().join(".ManualAid").join("config.toml"))
        .expect("config should exist");
    let table: toml::Table = toml::from_str(&content).expect("parse config");
    let key = skill.to_string_lossy().replace('\\', "/");
    assert_eq!(table["skill"][&key], toml::Value::Boolean(false));

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(!get_skill("project-.claude-s").expect("skill").is_enabled);
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
    let config_path = root.path().join(".ManualAid").join("config.toml");
    assert!(config_path.is_file());
    let first_content =
        std::fs::read_to_string(&config_path).expect("read config after first write");
    assert!(first_content.contains("# ManualAid 项目配置文件"));
    assert!(first_content.contains("[privacy_mask_extension.regex]"));
    assert!(first_content.contains("[privacy_mask_extension.literal]"));

    set_enabled(&second, false).expect("disable should succeed");
    let content = std::fs::read_to_string(&config_path).expect("read config");
    let table: toml::Table = toml::from_str(&content).expect("parse config");
    let skill = &table["skill"];
    let key_first = first.to_string_lossy().replace('\\', "/");
    let key_second = second.to_string_lossy().replace('\\', "/");
    assert_eq!(skill[&key_first], toml::Value::Boolean(false));
    assert_eq!(skill[&key_second], toml::Value::Boolean(false));
    assert!(table.contains_key("privacy_mask_extension"));
}

/// `set_enabled` preserves the comments and privacy tables of the default
/// project template while updating the `[skill]` table.
/// `set_enabled` 更新 `[skill]` 表时保留默认项目模板中的注释与 privacy 表。
#[test]
fn set_enabled_preserves_template_comments() {
    let _restore = lock_skills();
    let root = TempDir::new("preserve-template-comments");
    let home = TempDir::new("preserve-template-comments-home");
    let skill = write_skill(&root.path().join(".claude"), "s", Some("s"), "desc", "body");
    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");

    let config_dir = root.path().join(".ManualAid");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    std::fs::write(
        config_dir.join("config.toml"),
        DEFAULT_PROJECT_CONFIG_CONTENT,
    )
    .expect("write config");

    set_enabled(&skill, false).expect("disable should succeed");
    let content = std::fs::read_to_string(config_dir.join("config.toml")).expect("read config");
    assert!(content.contains("# ManualAid 项目配置文件"));
    assert!(content.contains("# 隐私掩码扩展 —— 正则匹配"));
    assert!(content.contains("# ExamApiKey = \"^sk-[A-Za-z0-9]{7}$\""));
    assert!(content.contains("# UserName = \"Alice\""));

    let table: toml::Table = toml::from_str(&content).expect("parse config");
    assert!(table.contains_key("privacy_mask_extension"));
    let key = skill.to_string_lossy().replace('\\', "/");
    assert_eq!(table["skill"][&key], toml::Value::Boolean(false));
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
    assert!(get_skill("global-.codex-g").expect("skill").is_enabled);

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    assert!(get_skill("global-.codex-g").expect("skill").is_enabled);
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
    assert!(get_skill("project-.claude-s").expect("skill").is_enabled);
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

    assert!(!get_skill("project-.claude-rel").expect("skill").is_enabled);
}

/// The exposed names of the enabled skills shorten and lengthen as the
/// enabled set changes (issue example 2 end to end).
/// 已启用技能的暴露名随启用集合变化而缩短或加长（issue 示例 2 端到端）。
#[test]
fn reload_skills_exposes_shortest_unique_names() {
    let _restore = lock_skills();
    let root = TempDir::new("exposed-root");
    let home = TempDir::new("exposed-home");
    write_skill(
        &root.path().join(".agents"),
        "pdf",
        Some("pdf"),
        "project pdf",
        "p",
    );
    let global_pdf = write_skill(
        &home.path().join(".agents"),
        "pdf",
        Some("pdf"),
        "global pdf",
        "g",
    );

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    // Only the project copy is enabled: the plain name is exposed.
    // 只有项目副本启用：暴露裸名。
    assert_eq!(
        resolve_skill("pdf").expect("enabled copy").description,
        "project pdf"
    );

    set_enabled(&global_pdf, true).expect("enable global copy");
    let enabled = enabled_skills();
    let map = exposed_name_map(&enabled);
    assert_eq!(enabled.len(), 2);
    assert_eq!(
        map.get("project-.agents-pdf").map(String::as_str),
        Some("project-.agents-pdf")
    );
    assert_eq!(
        map.get("global-.agents-pdf").map(String::as_str),
        Some("global-.agents-pdf")
    );

    set_enabled(&global_pdf, false).expect("disable global copy");
    assert_eq!(
        resolve_skill("pdf").expect("enabled copy").description,
        "project pdf"
    );
}

/// Identical content written with LF versus CRLF line endings in the body is
/// still deduplicated (project copy kept).
/// 正文使用 LF 与 CRLF 行尾的相同内容仍会被去重（保留项目副本）。
#[test]
fn reload_skills_dedups_across_line_endings() {
    let _restore = lock_skills();
    let root = TempDir::new("line-endings-root");
    let home = TempDir::new("line-endings-home");
    write_skill(
        &root.path().join(".claude"),
        "dup",
        Some("dup"),
        "same",
        "line one\nline two",
    );
    // The global copy keeps an LF frontmatter but a CRLF body, as produced
    // by Windows editors rewriting only part of the file.
    // 全局副本保留 LF frontmatter、CRLF 正文，模拟 Windows 编辑器仅改写
    // 文件部分内容的产物。
    let global_dir = home.path().join(".codex").join("skills").join("dup");
    std::fs::create_dir_all(&global_dir).expect("create global skill dir");
    std::fs::write(
        global_dir.join("SKILL.md"),
        "---\nname: dup\ndescription: same\n---\nline one\r\nline two",
    )
    .expect("write global SKILL.md");

    reload_skills_with_home(root.path(), home.path()).expect("reload should succeed");
    let skills = all_skills();
    assert_eq!(skills.len(), 1);
    assert!(!skills[0].is_global);
    assert_eq!(
        skills[0].path,
        root.path().join(".claude").join("skills").join("dup")
    );
}
