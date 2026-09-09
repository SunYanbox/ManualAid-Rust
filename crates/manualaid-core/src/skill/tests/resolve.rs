//! Exposed-name mapping and resolution tests for the naming rules in issue
//! #112.
//! 暴露名映射与解析测试，覆盖 issue #112 的命名规则。

use std::path::PathBuf;

use crate::skill::{Skill, exposed_name_map, resolve_in};

/// Build a scanner-shaped skill with explicit fields; the unique name is
/// passed as-is because these tests exercise hand-picked collision sets.
/// 构造字段显式的扫描器风格技能；唯一名称原样传入，因为测试针对精心
/// 挑选的冲突集合。
fn sk(unique_name: &str, name: &str, agent_dir: &str, is_global: bool, is_enabled: bool) -> Skill {
    Skill {
        unique_name: unique_name.to_string(),
        name: name.to_string(),
        description: "desc".to_string(),
        body: "body".to_string(),
        path: PathBuf::from(format!("/tmp/{unique_name}")),
        agent_dir: agent_dir.to_string(),
        is_global,
        is_enabled,
    }
}

#[test]
fn exposed_names_stay_plain_without_collision() {
    // Issue example 1: distinct plain names among the enabled skills are
    // exposed as-is.
    // issue 示例 1：启用技能中裸名互不冲突时原样暴露。
    let skills = vec![
        sk(
            "project-.agents-find-skills",
            "find-skills",
            ".agents",
            false,
            true,
        ),
        sk("project-.agents-pdf", "pdf", ".agents", false, true),
        sk("project-.claude-word", "word", ".claude", false, true),
    ];
    let map = exposed_name_map(&skills);
    assert_eq!(
        map.get("project-.agents-find-skills").map(String::as_str),
        Some("find-skills")
    );
    assert_eq!(
        map.get("project-.agents-pdf").map(String::as_str),
        Some("pdf")
    );
    assert_eq!(
        map.get("project-.claude-word").map(String::as_str),
        Some("word")
    );
}

#[test]
fn exposed_names_use_full_form_for_same_dir_across_scopes() {
    // Issue example 2: the same agent dir and name in both scopes cannot be
    // told apart by the dir prefix alone, so the full stable name is exposed.
    // issue 示例 2：同一 agent 目录与名称同时来自项目与全局范围时，仅目录
    // 前缀不足以区分，故暴露完整稳定名。
    let skills = vec![
        sk("project-.agents-pdf", "pdf", ".agents", false, true),
        sk("global-.agents-pdf", "pdf", ".agents", true, true),
    ];
    let map = exposed_name_map(&skills);
    assert_eq!(
        map.get("project-.agents-pdf").map(String::as_str),
        Some("project-.agents-pdf")
    );
    assert_eq!(
        map.get("global-.agents-pdf").map(String::as_str),
        Some("global-.agents-pdf")
    );
}

#[test]
fn exposed_names_add_dir_for_cross_dir_collision() {
    // Issue example 3: the same name in different agent dirs is exposed with
    // the agent dir prefix.
    // issue 示例 3：同一名称出现在不同 agent 目录时，暴露名带上目录前缀。
    let skills = vec![
        sk("project-.agents-word", "word", ".agents", false, true),
        sk("project-.claude-word", "word", ".claude", false, true),
    ];
    let map = exposed_name_map(&skills);
    assert_eq!(
        map.get("project-.agents-word").map(String::as_str),
        Some(".agents-word")
    );
    assert_eq!(
        map.get("project-.claude-word").map(String::as_str),
        Some(".claude-word")
    );
}

#[test]
fn exposed_names_ignore_disabled_skills() {
    // A disabled copy of the same name must not force prefixes on the
    // enabled one; callers pass the enabled subset.
    // 同名但禁用的副本不应迫使已启用者使用前缀；调用方传入的是已启用子集。
    let all = vec![
        sk("project-.agents-pdf", "pdf", ".agents", false, true),
        sk("global-.agents-pdf", "pdf", ".agents", true, false),
    ];
    let enabled: Vec<Skill> = all.into_iter().filter(|s| s.is_enabled).collect();
    let map = exposed_name_map(&enabled);
    assert_eq!(map.len(), 1);
    assert_eq!(
        map.get("project-.agents-pdf").map(String::as_str),
        Some("pdf")
    );
}

#[test]
fn exposed_names_fallback_when_plain_matches_prefixed() {
    // A skill literally named `.claude-pdf` (unique plain name) collides with
    // the dir-prefixed exposed name of the `.claude` group; every holder of a
    // duplicated exposed name falls back to its stable name.
    // 一个字面名为 `.claude-pdf` 的技能（裸名唯一）与 `.claude` 组的前缀
    // 暴露名相撞；所有持有重复暴露名的技能回退到各自的稳定名。
    let skills = vec![
        sk("project-.claude-pdf", "pdf", ".claude", false, true),
        sk("project-.agents-pdf", "pdf", ".agents", false, true),
        sk(
            "project-.codex-.claude-pdf",
            ".claude-pdf",
            ".codex",
            false,
            true,
        ),
    ];
    let map = exposed_name_map(&skills);
    assert_eq!(
        map.get("project-.claude-pdf").map(String::as_str),
        Some("project-.claude-pdf")
    );
    assert_eq!(
        map.get("project-.agents-pdf").map(String::as_str),
        Some(".agents-pdf")
    );
    assert_eq!(
        map.get("project-.codex-.claude-pdf").map(String::as_str),
        Some("project-.codex-.claude-pdf")
    );
}

#[test]
fn resolve_in_matches_stable_name_including_disabled() {
    let skills = vec![sk("project-.claude-a", "a", ".claude", false, false)];
    let hit = resolve_in(&skills, "project-.claude-a").expect("stable name matches");
    assert_eq!(hit.unique_name, "project-.claude-a");
}

#[test]
fn resolve_in_matches_exposed_and_plain_names() {
    let skills = vec![
        sk("project-.claude-word", "word", ".claude", false, true),
        sk("project-.agents-word", "word", ".agents", false, true),
        sk("global-.agents-pdf", "pdf", ".agents", true, false),
    ];
    // A plain name shared by two enabled skills is ambiguous.
    // 两个已启用技能共享的裸名存在歧义。
    assert_eq!(resolve_in(&skills, "word"), None);
    // The dir-prefixed exposed names resolve to their skills.
    // 目录前缀形式的暴露名可解析到对应技能。
    let hit = resolve_in(&skills, ".claude-word").expect("exposed name matches");
    assert_eq!(hit.unique_name, "project-.claude-word");
    let hit = resolve_in(&skills, ".agents-word").expect("exposed name matches");
    assert_eq!(hit.unique_name, "project-.agents-word");
    // A plain name held by exactly one loaded skill resolves even when that
    // skill is disabled, so callers can report "disabled by the user".
    // 仅一个已加载技能持有的裸名可解析，即使该技能已禁用，调用方从而能
    // 报告“已被用户禁用”。
    let hit = resolve_in(&skills, "pdf").expect("unique plain name matches");
    assert_eq!(hit.unique_name, "global-.agents-pdf");
}

#[test]
fn resolve_in_unknown_name_returns_none() {
    let skills = vec![sk("project-.claude-a", "a", ".claude", false, true)];
    assert_eq!(resolve_in(&skills, "nope"), None);
    assert_eq!(resolve_in(&skills, ".codex-a"), None);
}
