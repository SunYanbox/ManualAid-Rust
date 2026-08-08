//! Integration tests for the `manualaid-cli` library API.
//! `manualaid-cli` 库公共 API 的集成测试。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use manualaid_cli::dir_tree::{
    DEFAULT_VIEW_DEPTH, DEFAULT_VIEW_LIMIT, DirViewConfig, format_dir_tree,
};
use manualaid_cli::{
    DESCRIPTION_MAX_CHARS, SkillScope, filter_skills, format_bytes, format_default_output,
    format_duration, format_error_output, format_mask_output, format_restore_output, format_skill,
    format_skill_list, format_skill_output, format_timings, load_skills, mask, mask_with_chars,
    pager, read_input, restore, restore_with_chars, skill_source_path, style, t_fmt,
    truncate_description,
};
use manualaid_core::error::CoreError;
use manualaid_core::privacy::{PrivacyMaskExtension, PrivacyMasker, restore_masked_data};
use manualaid_core::skill::{Skill, all_skills, reload_skills_with_home, reset_skills};
use manualaid_core::user_dir;

mod common;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that depend on the process-wide styling switch.
/// 串行化依赖进程级样式开关的测试。
static STYLE_LOCK: Mutex<()> = Mutex::new(());

fn locale_guard() -> MutexGuard<'static, ()> {
    LOCALE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn style_guard() -> MutexGuard<'static, ()> {
    STYLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
fn read_input_reads_existing_file() {
    let tmp = common::TempDir::new("read-file");
    let path = tmp.path().join("input.txt");
    fs::write(&path, "file content").unwrap();
    assert_eq!(read_input(path.to_str().unwrap()).unwrap(), "file content");
}

#[test]
fn read_input_directory_is_invalid_path() {
    let tmp = common::TempDir::new("read-dir");
    assert!(matches!(
        read_input(tmp.path().to_str().unwrap()),
        Err(CoreError::InvalidPath(_))
    ));
}

#[test]
fn read_input_missing_path_is_literal_text() {
    assert_eq!(read_input("not a real path").unwrap(), "not a real path");
}

#[test]
fn mask_hides_plaintext_and_roundtrips() {
    let masker = PrivacyMasker::new().unwrap();
    let input = "mail me at bob@example.com";
    let (masked, snapshot) = mask(&masker, input).unwrap();
    assert!(masked.contains("[PRV_EMAIL_"));
    assert!(!masked.contains("bob@example.com"));
    assert_eq!(snapshot.values().next().unwrap(), "bob@example.com");

    let mapping: HashMap<String, String> = snapshot.clone().into_iter().collect();
    assert_eq!(restore_masked_data(&masked, &mapping), input);
}

#[test]
fn mask_uses_config_extensions_with_fake_home() {
    let tmp = common::TempDir::new("mask-ext");
    let home = tmp.path().join("home");
    let project = tmp.path().join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();
    fs::write(
        project.join(".ManualAid").join("config.toml"),
        "[privacy_mask_extension.literal]\nUserName = \"Alice\"\n",
    )
    .unwrap();

    let extensions = PrivacyMaskExtension::load_with_home(&project, &home).unwrap();
    let masker = PrivacyMasker::from_extensions(&extensions).unwrap();
    let (masked, snapshot) = mask(&masker, "hi Alice").unwrap();
    assert!(masked.contains("[PRV_UserName_"));
    assert!(!masked.contains("Alice"));
    let mapping: HashMap<String, String> = snapshot.into_iter().collect();
    assert_eq!(restore_masked_data(&masked, &mapping), "hi Alice");
}

#[test]
fn mask_snapshot_keys_are_sorted() {
    let masker = PrivacyMasker::new().unwrap();
    let (masked, snapshot) = mask(&masker, "a@example.com b@example.com").unwrap();
    let keys: Vec<&String> = snapshot.keys().collect();
    assert_eq!(keys.len(), 2);
    assert!(keys[0] < keys[1]);
    assert!(keys[0].starts_with("[PRV_EMAIL_"));
    assert!(keys[1].starts_with("[PRV_EMAIL_"));
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(json.find(keys[0]).unwrap() < json.find(keys[1]).unwrap());
    assert!(!masked.contains("a@example.com"));
    assert!(!masked.contains("b@example.com"));
}

#[test]
fn restore_roundtrip_via_snapshot_file() {
    let tmp = common::TempDir::new("restore");
    let masker = PrivacyMasker::new().unwrap();
    let input = "call +1 555 010 1234 now";
    let (masked, snapshot) = mask(&masker, input).unwrap();

    let snapshot_path = tmp.path().join("snapshot.json");
    fs::write(
        &snapshot_path,
        serde_json::to_string_pretty(&snapshot).unwrap(),
    )
    .unwrap();

    let restored = restore(&masked, &snapshot_path).unwrap();
    assert_eq!(restored, input);
}

#[test]
fn restore_missing_snapshot_is_io_error() {
    let tmp = common::TempDir::new("restore-missing");
    let err = restore("[PRV_EMAIL_1]", &tmp.path().join("missing.json")).unwrap_err();
    assert!(matches!(err, CoreError::Io(_) | CoreError::NotFound(_)));
}

#[test]
fn restore_invalid_snapshot_json_is_parse_error() {
    let tmp = common::TempDir::new("restore-invalid");
    let path = tmp.path().join("snapshot.json");
    fs::write(&path, "not json").unwrap();
    let err = restore("[PRV_EMAIL_1]", &path).unwrap_err();
    assert!(matches!(err, CoreError::Parse(_)));
}

#[test]
fn t_fmt_replaces_placeholders() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let name_line = t_fmt("cli.skill.unique_name", &[("unique_name", "u")]);
    assert!(name_line.contains("Unique name: u"));
    let chars_line = t_fmt("cli.skill.desc_chars_total", &[("chars", "42")]);
    assert!(chars_line.contains("Total chars: 42"));
    assert!(!name_line.contains("%{"));
    assert!(!chars_line.contains("%{"));
}

#[test]
fn t_fmt_leaves_unknown_placeholders_untouched() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let line = t_fmt("cli.skill.unique_name", &[("name", "n")]);
    assert!(line.contains("%{unique_name}"));
    assert!(!line.contains("%{name}"));
}

#[test]
fn format_skill_uses_localized_template_and_char_count() {
    let _guard = locale_guard();
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
    let _guard = locale_guard();
    let _style = style_guard();
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
    let _guard = locale_guard();
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
    let _guard = locale_guard();
    i18n::set_locale("zh-CN");
    let a = skill_with_path("a", "/home/u/.codex/skills/a");
    let b = skill_with_path("b", "/home/u/.claude/skills/b");
    let out = format_skill_list(&[a, b]);
    assert!(out.contains("- /home/u/.codex\n  - 唯一名称：a"));
    assert!(out.contains("\n\n- /home/u/.claude"));
}

#[test]
fn format_default_output_plain_and_styled() {
    let _guard = locale_guard();
    let _style = style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(
        format_default_output("ManualAid running..."),
        "ManualAid running...\n"
    );
    style::set_enabled(true);
    assert_eq!(
        format_default_output("ManualAid running..."),
        "\n\x1b[32mManualAid running...\x1b[0m\n"
    );
    style::set_enabled(false);
}

#[test]
fn format_mask_output_has_two_headed_sections() {
    let _guard = locale_guard();
    let _style = style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    let out = format_mask_output("contact [PRV_EMAIL_1]", "{\n  \"k\": \"v\"\n}");
    assert!(out.starts_with("\nMasked text\ncontact [PRV_EMAIL_1]\n\nSnapshot JSON\n"));
    assert!(out.ends_with("\"v\"\n}\n"));
    style::set_enabled(true);
    let out = format_mask_output("m", "{}");
    assert!(out.contains("\x1b[1;36mMasked text\x1b[0m\nm"));
    assert!(out.contains("\x1b[1;36mSnapshot JSON\x1b[0m\n{}"));
    style::set_enabled(false);
}

#[test]
fn format_restore_output_plain_and_styled() {
    let _guard = locale_guard();
    let _style = style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(format_restore_output("hello"), "hello\n");
    assert_eq!(format_restore_output(""), "");
    style::set_enabled(true);
    assert_eq!(
        format_restore_output("hello"),
        "\n\x1b[1;36mRestored text\x1b[0m\n\x1b[32mhello\x1b[0m\n"
    );
    assert_eq!(format_restore_output(""), "");
    style::set_enabled(false);
}

#[test]
fn format_skill_output_plain_and_styled() {
    let _guard = locale_guard();
    let _style = style_guard();
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

#[test]
fn format_error_output_plain_and_styled() {
    let _guard = locale_guard();
    let _style = style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(
        format_error_output("Masking failed: x"),
        "Masking failed: x\n"
    );
    style::set_enabled(true);
    assert_eq!(
        format_error_output("Masking failed: x"),
        "\x1b[1;31mError: Masking failed: x\x1b[0m\n"
    );
    style::set_enabled(false);
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
    let _guard = locale_guard();
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
fn snapshot_json_serializes_btree_deterministically() {
    let mut snapshot = BTreeMap::new();
    snapshot.insert("[PRV_EMAIL_2]".to_string(), "b@example.com".to_string());
    snapshot.insert("[PRV_EMAIL_1]".to_string(), "a@example.com".to_string());
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    assert!(json.find("[PRV_EMAIL_1]").unwrap() < json.find("[PRV_EMAIL_2]").unwrap());
}

#[test]
fn pager_prints_all_when_not_terminal() {
    pager::print_paged("line one\nline two\n").expect("print should succeed");
}

#[test]
fn collapsed_pager_prints_all_when_not_terminal() {
    let long = (1..=100)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    pager::print_paged_collapsed(&long).expect("print should succeed");
}

#[test]
fn format_duration_is_milliseconds_with_nanosecond_precision() {
    assert_eq!(format_duration(Duration::ZERO), "0.000000 ms");
    assert_eq!(format_duration(Duration::from_nanos(5)), "0.000005 ms");
    assert_eq!(
        format_duration(Duration::from_nanos(1_234_567)),
        "1.234567 ms"
    );
    assert_eq!(format_duration(Duration::from_secs(2)), "2000.000000 ms");
}

#[test]
fn format_bytes_auto_selects_unit_with_three_decimals() {
    assert_eq!(format_bytes(0), "0.000 KB");
    assert_eq!(format_bytes(512), "0.500 KB");
    assert_eq!(format_bytes(1536), "1.500 KB");
    assert_eq!(format_bytes(1_572_864), "1.500 MB");
    assert_eq!(format_bytes(1_073_741_824), "1.000 GB");
}

#[test]
fn format_timings_renders_heading_and_lines() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    assert_eq!(format_timings(&[]), "");
    let out = format_timings(&[
        "Mask: 1.234567 ms (25 chars)".to_string(),
        "Init: 0.000123 ms".to_string(),
    ]);
    assert!(
        out.starts_with("\nTimings\n  - Mask: 1.234567 ms (25 chars)\n  - Init: 0.000123 ms\n")
    );
    assert!(out.ends_with('\n'));
}

#[test]
fn mask_with_chars_returns_input_char_count() {
    let masker = PrivacyMasker::new().unwrap();
    let (masked, snapshot, chars) = mask_with_chars(&masker, "mail me at bob@example.com").unwrap();
    assert_eq!(chars, 26);
    assert!(masked.contains("[PRV_EMAIL_"));
    assert!(!masked.contains("bob@example.com"));
    assert!(snapshot.values().any(|value| value == "bob@example.com"));
}

#[test]
fn restore_with_chars_returns_input_char_count() {
    let tmp = common::TempDir::new("restore-chars");
    let snapshot = tmp.path().join("snapshot.json");
    fs::write(&snapshot, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();
    let (restored, chars) = restore_with_chars("contact [PRV_EMAIL_1]", &snapshot).unwrap();
    assert_eq!(restored, "contact jane@example.com");
    assert_eq!(chars, 21);
}

#[test]
fn dir_view_config_defaults() {
    let config = DirViewConfig::default();
    assert_eq!(config.depth, Some(DEFAULT_VIEW_DEPTH));
    assert_eq!(config.per_level_limit, Some(DEFAULT_VIEW_LIMIT));
}

fn view_config(depth: Option<usize>, limit: Option<usize>) -> DirViewConfig {
    DirViewConfig {
        depth,
        per_level_limit: limit,
    }
}

#[test]
fn format_dir_tree_missing_root_is_not_found() {
    let tmp = common::TempDir::new("tree-missing");
    let err = format_dir_tree(&tmp.path().join(".ManualAid"), &DirViewConfig::default())
        .expect_err("tree should fail");
    assert!(matches!(err, CoreError::NotFound(_)));
}

#[test]
fn format_dir_tree_file_root_is_invalid_path() {
    let tmp = common::TempDir::new("tree-file");
    let path = tmp.path().join("blocker");
    fs::write(&path, "file").unwrap();
    let err = format_dir_tree(&path, &DirViewConfig::default()).expect_err("tree should fail");
    assert!(matches!(err, CoreError::InvalidPath(_)));
}

#[test]
fn format_dir_tree_empty_dir_prints_only_the_root() {
    let tmp = common::TempDir::new("tree-empty");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    let out = format_dir_tree(&root, &view_config(None, None)).unwrap();
    assert_eq!(out, format!("- {}", root.display()));
}

#[test]
fn format_dir_tree_sorts_dirs_first() {
    let tmp = common::TempDir::new("tree-sort");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("bb")).unwrap();
    fs::create_dir_all(root.join("aa")).unwrap();
    fs::write(root.join("zz.txt"), "").unwrap();
    fs::write(root.join("aa.txt"), "").unwrap();
    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], format!("- {}", root.display()));
    assert!(lines[1].ends_with("aa/"));
    assert!(lines[2].ends_with("bb/"));
    assert!(lines[3].ends_with("aa.txt"));
    assert!(lines[4].ends_with("zz.txt"));
}

#[test]
fn format_dir_tree_limits_files_per_level() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-file-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..10 {
        fs::write(root.join(format!("f{i}.txt")), "").unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), Some(7))).unwrap();
    assert!(out.contains("f6.txt"));
    assert!(!out.contains("f7.txt"));
    assert!(out.contains("… 3 more files"));
}

#[test]
fn format_dir_tree_limits_dirs_per_level() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-dir-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..8 {
        fs::create_dir_all(root.join(format!("d{i}"))).unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), Some(2))).unwrap();
    assert!(out.contains("d5/"));
    assert!(!out.contains("d6/"));
    assert!(out.contains("… 2 more dirs"));
}

#[test]
fn format_dir_tree_unlimited_limit_shows_everything() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-no-limit");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..12 {
        fs::write(root.join(format!("f{i}.txt")), "").unwrap();
    }
    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    assert!(out.contains("f11.txt"));
    assert!(!out.contains("more"));
}

#[test]
fn format_dir_tree_depth_controls_recursion() {
    let tmp = common::TempDir::new("tree-depth");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("a").join("b")).unwrap();
    fs::write(root.join("a").join("b").join("c.txt"), "").unwrap();

    let out = format_dir_tree(&root, &view_config(Some(0), None)).unwrap();
    assert_eq!(out, format!("- {}", root.display()));

    let out = format_dir_tree(&root, &view_config(Some(1), None)).unwrap();
    assert!(out.contains("a/"));
    assert!(!out.contains("b/"));

    let out = format_dir_tree(&root, &view_config(Some(2), None)).unwrap();
    assert!(out.contains("b/"));
    assert!(!out.contains("c.txt"));

    let out = format_dir_tree(&root, &view_config(None, None)).unwrap();
    assert!(out.contains("c.txt"));
}

#[test]
fn format_dir_tree_global_dir_budget_is_depth_first() {
    let _guard = locale_guard();
    i18n::set_locale("en");
    let tmp = common::TempDir::new("tree-global-budget");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(&root).unwrap();
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                for l in 0..3 {
                    fs::create_dir_all(root.join(format!("d{i}/d{j}/d{k}/d{l}"))).unwrap();
                }
            }
        }
    }
    // limit 1: per-level dir cap 3, whole-tree cap 64; the tree has
    // 3 + 9 + 27 + 81 = 120 non-root dirs.
    let out = format_dir_tree(&root, &view_config(None, Some(1))).unwrap();
    let shown = out
        .lines()
        .filter(|line| line.contains("└── ") || line.contains("├── "))
        .filter(|line| line.ends_with('/'))
        .count();
    assert_eq!(shown, 64);
    assert!(out.contains("… 3 more dirs"));
    assert!(out.contains("… 2 more dirs"));
}

#[test]
fn format_dir_tree_keeps_connector_continuation_for_children() {
    let tmp = common::TempDir::new("tree-connectors");
    let root = tmp.path().join(".ManualAid");
    fs::create_dir_all(root.join("a")).unwrap();
    fs::write(root.join("a").join("x.txt"), "").unwrap();
    fs::write(root.join("a").join("y.txt"), "").unwrap();
    fs::write(root.join("z.txt"), "").unwrap();
    let out = format_dir_tree(&root, &view_config(Some(2), None)).unwrap();
    assert!(out.contains("├── a/\n│   ├── x.txt\n│   └── y.txt"));
}
