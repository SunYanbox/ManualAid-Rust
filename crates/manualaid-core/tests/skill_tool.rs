//! Integration tests for the skill tool: loading an enabled skill, a
//! disabled skill and an unknown name.
//! Skill 工具集成测试：加载已启用技能、已禁用技能与未知名称。

use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;
use manualaid_core::skill::{reload_skills, reload_skills_with_home, reset_skills, set_enabled};
use manualaid_core::tools::ToolKind;
use serde_json::Value;

/// Serializes tests that mutate the process-global skill store.
/// 串行化修改进程级技能存储的测试。
static SKILL_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

fn temp_root(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-core-skill-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Write a project-scope skill folder and reload the skill store.
/// 写入项目范围的技能文件夹并重新加载技能存储。
fn setup_project_skill(root: &Path, folder: &str, description: &str) {
    let dir = root.join(".claude").join("skills").join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {folder}\ndescription: {description}\n---\nbody text"),
    )
    .unwrap();
    reload_skills(root).unwrap();
}

fn skill_params(name: &str) -> IndexMap<String, Value> {
    let mut params = IndexMap::new();
    params.insert("skill".to_string(), Value::String(name.to_string()));
    params
}

#[tokio::test]
async fn enabled_project_skill_returns_body() {
    let _guard = SKILL_LOCK.lock().await;
    let root = temp_root("enabled");
    setup_project_skill(&root, "demo-skill", "demo description");
    let skills = manualaid_core::skill::all_skills();
    let skill = skills
        .iter()
        .find(|skill| skill.unique_name == "project-.claude-demo-skill")
        .expect("skill loaded");
    let expected_path = skill.path.to_string_lossy().replace('\\', "/");

    // Both the exposed plain name and the full stable unique name work.
    // 暴露裸名与完整稳定唯一名称都可调用。
    for name in ["demo-skill", "project-.claude-demo-skill"] {
        let result = ToolKind::Skill.run(&skill_params(name)).await;
        assert!(result.success, "{}", result.output);
        assert!(result.output.contains("body text"));
        assert!(result.output.contains("invoke_skill"));
        assert!(result.output.contains("\"path\""));
        assert!(result.output.contains(&expected_path));
    }
    reset_skills();
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn disabled_skill_is_rejected() {
    let _guard = SKILL_LOCK.lock().await;
    let root = temp_root("disabled");
    setup_project_skill(&root, "off-skill", "off description");
    let skills = manualaid_core::skill::all_skills();
    let skill = skills
        .iter()
        .find(|skill| skill.unique_name == "project-.claude-off-skill")
        .expect("skill loaded");
    set_enabled(&skill.path, false).unwrap();

    // A disabled skill is reported as such whether it is addressed by its
    // stable unique name or its (now unresolvable) plain name.
    // 无论以稳定唯一名称还是（现已无法解析的）裸名寻址，禁用技能都报
    // “disabled”。
    for name in ["project-.claude-off-skill", "off-skill"] {
        let result = ToolKind::Skill.run(&skill_params(name)).await;
        assert!(!result.success);
        assert!(result.output.contains("disabled"));
    }
    reset_skills();
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unknown_skill_lists_enabled_alternatives() {
    let _guard = SKILL_LOCK.lock().await;
    let root = temp_root("unknown");
    setup_project_skill(&root, "known-skill", "known description");
    let result = ToolKind::Skill.run(&skill_params("missing-skill")).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
    assert!(result.output.contains("known-skill"));
    reset_skills();
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unknown_skill_lists_exposed_names_for_collisions() {
    let _guard = SKILL_LOCK.lock().await;
    let root = temp_root("unknown-collision");
    let home = temp_root("unknown-collision-home");
    let project_dir = root.join(".claude").join("skills").join("pdf");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(
        project_dir.join("SKILL.md"),
        "---\nname: pdf\ndescription: project pdf\n---\nbody",
    )
    .unwrap();
    let global_dir = home.join(".codex").join("skills").join("pdf");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(
        global_dir.join("SKILL.md"),
        "---\nname: pdf\ndescription: global pdf\n---\nbody",
    )
    .unwrap();
    reload_skills_with_home(&root, &home).unwrap();
    let global = manualaid_core::skill::all_skills()
        .into_iter()
        .find(|skill| skill.unique_name == "global-.codex-pdf")
        .expect("global skill loaded");
    set_enabled(&global.path, true).unwrap();

    // Both copies are enabled, so the alternatives list uses the
    // dir-prefixed exposed names instead of the ambiguous plain name.
    // 两个副本都已启用，备选列表使用目录前缀暴露名而非有歧义的裸名。
    let result = ToolKind::Skill.run(&skill_params("pdf")).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
    assert!(result.output.contains(".claude-pdf"));
    assert!(result.output.contains(".codex-pdf"));
    assert!(!result.output.contains(" pdf"));
    reset_skills();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
}
