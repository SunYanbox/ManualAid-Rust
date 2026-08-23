//! API tests: skill formatting, scoping and scanning.
//! API 测试：技能格式化、范围过滤与扫描。

use std::fs;
use std::path::PathBuf;

use manualaid_cli::{
    DESCRIPTION_MAX_CHARS, SkillScope, filter_skills, format_skill, format_skill_list,
    format_skill_output, load_skills, skill_source_path, style, truncate_description,
};
use manualaid_core::skill::{Skill, all_skills, reload_skills_with_home, reset_skills};
use manualaid_core::user_dir;

use super::common;

fn skill(unique_name: &str, is_global: bool, description: &str) -> Skill {
    Skill {
        unique_name: unique_name.to_string(),
        name: unique_name.to_string(),
        description: description.to_string(),
        body: String::new(),
        path: PathBuf::from(format!("/tmp/{unique_name}")),
        is_global,
        is_enabled: !is_global,
    }
}

fn skill_with_path(unique_name: &str, path: &str) -> Skill {
    Skill {
        unique_name: unique_name.to_string(),
        name: unique_name.to_string(),
        description: "desc".to_string(),
        body: String::new(),
        path: PathBuf::from(path),
        is_global: false,
        is_enabled: true,
    }
}

#[test]
fn format_skill_uses_localized_template_and_char_count() {
    let _guard = super::locale_guard();
    i18n::set_locale("zh-CN");
    let long = "a".repeat(101);
    let s = skill("uniq", false, &long);
    let block = format_skill(&s);
    assert!(block.contains("  - 唯一名称：uniq"));
    assert!(block.contains("    - 名称：uniq"));
    assert!(block.contains("    - 描述："));
    assert!(block.contains("…"));
    assert!(block.contains("    - 总字符数：101"));

    i18n::set_locale("en");
    let block = format_skill(&s);
    assert!(block.contains("  - Unique name: uniq"));
    assert!(block.contains("    - Total chars: 101"));
}

#[test]
fn format_skill_applies_styles_when_enabled() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    let s = skill("uniq", false, "desc");
    style::set_enabled(true);
    let block = format_skill(&s);
    assert!(block.contains("\x1b[1m  - Unique name: uniq\x1b[0m"));
    assert!(block.contains("    - Name: uniq"));
    assert!(block.contains("\x1b[90m    - Total chars: 4\x1b[0m"));
    style::set_enabled(false);
}

#[test]
fn skill_source_path_is_parent_of_skills_dir() {
    let s = skill_with_path(
        "theme-factory",
        "C:/Users/alice/.cc-switch/skills/theme-factory",
    );
    assert_eq!(
        skill_source_path(&s),
        PathBuf::from("C:/Users/alice/.cc-switch")
    );
}

#[test]
fn format_skill_list_groups_by_source() {
    let _guard = super::locale_guard();
    i18n::set_locale("zh-CN");
    let a = skill_with_path("a", "/home/u/.codex/skills/a");
    let b = skill_with_path("b", "/home/u/.codex/skills/b");
    let c = skill_with_path("c", "/home/u/.claude/skills/c");
    let out = format_skill_list(&[a, b, c]);
    assert_eq!(out.matches("- /home/u/.codex").count(), 1);
    assert_eq!(out.matches("- /home/u/.claude").count(), 1);
    assert!(out.contains("  - 唯一名称：a"));
    assert!(out.contains("  - 唯一名称：b"));
    assert!(out.contains("  - 唯一名称：c"));
}

#[test]
fn format_skill_list_separates_groups_with_blank_lines() {
    let _guard = super::locale_guard();
    i18n::set_locale("zh-CN");
    let a = skill_with_path("a", "/home/u/.codex/skills/a");
    let b = skill_with_path("b", "/home/u/.claude/skills/b");
    let out = format_skill_list(&[a, b]);
    assert!(out.contains("- /home/u/.codex\n  - 唯一名称：a"));
    assert!(out.contains("\n\n- /home/u/.claude"));
}

#[test]
fn truncate_description_boundary() {
    let at_limit = "a".repeat(DESCRIPTION_MAX_CHARS);
    assert_eq!(
        truncate_description(&at_limit, DESCRIPTION_MAX_CHARS),
        at_limit
    );
    let over = "a".repeat(DESCRIPTION_MAX_CHARS + 1);
    let truncated = truncate_description(&over, DESCRIPTION_MAX_CHARS);
    assert_eq!(truncated.chars().count(), DESCRIPTION_MAX_CHARS + 1);
    assert!(truncated.ends_with('…'));
    assert!(!truncated[..truncated.len() - "…".len()].contains('…'));
}

#[test]
fn filter_skills_scopes() {
    let project = skill("project-a", false, "project desc");
    let global = skill("global-a", true, "global desc");
    let all = vec![project.clone(), global.clone()];

    let filtered_all = filter_skills(all.clone(), SkillScope::All);
    assert_eq!(filtered_all.len(), 2);

    let filtered_global = filter_skills(all.clone(), SkillScope::Global);
    assert_eq!(filtered_global.len(), 1);
    assert!(filtered_global[0].is_global);

    let filtered_project = filter_skills(all.clone(), SkillScope::Project);
    assert_eq!(filtered_project.len(), 1);
    assert!(!filtered_project[0].is_global);
}

#[test]
fn skill_scan_filter_and_format_chain() {
    let _guard = super::locale_guard();
    let tmp = common::TempDir::new("skills-chain");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();
    common::write_skill(
        &project,
        ".claude",
        "projskill",
        Some("Proj"),
        "short project desc",
    );
    common::write_skill(&home, ".codex", "globskill", Some("Glob"), "global desc");

    reset_skills();
    reload_skills_with_home(&project, &home).expect("reload skills");
    let all = all_skills();
    let names: Vec<&str> = all.iter().map(|s| s.unique_name.as_str()).collect();
    assert!(names.contains(&"Proj"));
    assert!(names.contains(&"Glob"));

    let project_only = filter_skills(all.clone(), SkillScope::Project);
    assert!(project_only.iter().all(|s| !s.is_global));
    assert!(project_only.iter().any(|s| s.unique_name == "Proj"));

    let global_only = filter_skills(all.clone(), SkillScope::Global);
    assert!(global_only.iter().all(|s| s.is_global));
    assert!(global_only.iter().any(|s| s.unique_name == "Glob"));

    let proj = all.iter().find(|s| s.unique_name == "Proj").unwrap();
    i18n::set_locale("zh-CN");
    let block = format_skill(proj);
    assert!(block.contains("  - 唯一名称：Proj"));
    assert!(block.contains("    - 名称：Proj"));
    assert!(block.contains("    - 描述：short project desc"));
    assert!(block.contains("    - 总字符数：18"));

    let listed = format_skill_list(&all);
    assert!(listed.contains(&format!("- {}\n", project.join(".claude").display())));
    assert!(listed.contains(&format!("- {}\n", home.join(".codex").display())));

    if user_dir::home_dir().is_ok() {
        assert!(load_skills(&project).is_ok());
    } else {
        eprintln!("skipping load_skills: home directory cannot be resolved in this environment");
    }
    reset_skills();
}

#[test]
fn format_skill_output_plain_and_styled() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    let a = skill_with_path("a", "/home/u/.codex/skills/a");
    style::set_enabled(false);
    assert_eq!(format_skill_output(&[]), "");
    let out = format_skill_output(std::slice::from_ref(&a));
    assert!(out.starts_with("- /home/u/.codex\n"));
    assert!(out.ends_with("Total chars: 4\n"));
    style::set_enabled(true);
    let out = format_skill_output(&[a]);
    assert!(out.starts_with("\n\x1b[1;36mSkills (1)\x1b[0m\n\n"));
    assert!(out.contains("\x1b[1m  - Unique name: a\x1b[0m"));
    style::set_enabled(false);
}
