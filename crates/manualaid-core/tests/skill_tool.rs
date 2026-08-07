//! Integration tests for the skill tool: loading an enabled skill, a
//! disabled skill and an unknown name.
//! Skill 工具集成测试：加载已启用技能、已禁用技能与未知名称。

use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;
use manualaid_core::skill::{reload_skills, reset_skills, set_enabled};
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
    let result = ToolKind::Skill.run(&skill_params("demo-skill")).await;
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("body text"));
    assert!(result.output.contains("invoke_skill"));
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
        .find(|skill| skill.unique_name == "off-skill")
        .expect("skill loaded");
    set_enabled(&skill.path, false).unwrap();

    let result = ToolKind::Skill.run(&skill_params("off-skill")).await;
    assert!(!result.success);
    assert!(result.output.contains("disabled"));
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
