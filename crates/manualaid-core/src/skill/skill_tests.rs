use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use toml::Table;

use super::frontmatter::{Frontmatter, parse_frontmatter};
use super::*;

fn skill(name: &str, description: &str, body: &str, path: &str, is_global: bool) -> Skill {
    Skill {
        unique_name: name.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        path: PathBuf::from(path),
        is_global,
        is_enabled: !is_global,
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "manualaid-skill-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn fm(content: &str) -> Frontmatter {
    parse_frontmatter(content).expect("parse should succeed")
}

#[test]
fn parse_frontmatter_single_line_fields() {
    let frontmatter =
        fm("---\nname: greeter\ndescription: A greeting skill\n---\n## Usage\nHello\n");
    assert_eq!(frontmatter.name.as_deref(), Some("greeter"));
    assert_eq!(frontmatter.description, "A greeting skill");
    assert_eq!(frontmatter.body, "## Usage\nHello\n");
}

#[test]
fn parse_frontmatter_folded_block() {
    let frontmatter = fm("---\nname: a\ndescription: >\n  first line\n  second line\n---\nbody");
    assert_eq!(frontmatter.description, "first line second line");
}

#[test]
fn parse_frontmatter_folded_block_blank_line() {
    let frontmatter = fm("---\ndescription: >\n  first\n\n  second\n---\n");
    assert_eq!(frontmatter.description, "first\nsecond");
}

#[test]
fn parse_frontmatter_literal_block() {
    let frontmatter = fm("---\ndescription: |\n  line one\n  line two\n---\n");
    assert_eq!(frontmatter.description, "line one\nline two");
}

#[test]
fn parse_frontmatter_literal_block_blank_line() {
    let frontmatter = fm("---\ndescription: |\n  line one\n\n  line two\n---\n");
    assert_eq!(frontmatter.description, "line one\n\nline two");
}

#[test]
fn parse_frontmatter_block_followed_by_another_key() {
    let frontmatter = fm("---\ndescription: >\n  folded text\nname: after\n---\n");
    assert_eq!(frontmatter.description, "folded text");
    assert_eq!(frontmatter.name.as_deref(), Some("after"));
}

#[test]
fn parse_frontmatter_body_without_trailing_newline() {
    let frontmatter = fm("---\nname: a\ndescription: d\n---");
    assert_eq!(frontmatter.body, "");
}

#[test]
fn parse_frontmatter_indented_continuation() {
    let frontmatter = fm("---\ndescription:\n  first\n  second\n---\n");
    assert_eq!(frontmatter.description, "first second");
}

#[test]
fn parse_frontmatter_chomp_variants() {
    let folded = fm("---\ndescription: >-\n  a\n  b\n---\n");
    assert_eq!(folded.description, "a b");
    let literal = fm("---\ndescription: |+\n  a\n  b\n---\n");
    assert_eq!(literal.description, "a\nb");
}

#[test]
fn parse_frontmatter_no_frontmatter_returns_none() {
    assert!(parse_frontmatter("plain text").is_none());
}

#[test]
fn parse_frontmatter_unterminated_returns_none() {
    assert!(parse_frontmatter("---\nname: a\n").is_none());
}

#[test]
fn parse_frontmatter_missing_name_is_none_field() {
    let frontmatter = fm("---\ndescription: only desc\n---\nbody");
    assert_eq!(frontmatter.name, None);
    assert_eq!(frontmatter.description, "only desc");
}

#[test]
fn parse_frontmatter_empty_description_defaults_to_empty() {
    let frontmatter = fm("---\nname: a\n---\nbody");
    assert!(frontmatter.description.is_empty());
}

#[test]
fn parse_frontmatter_ignores_comments_lists_and_unknown_keys() {
    let frontmatter = fm("---\n# comment\n- list item\nname: a\nunknown: x\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
    assert_eq!(frontmatter.description, "d");
}

#[test]
fn parse_frontmatter_ignores_lines_without_colon() {
    let frontmatter = fm("---\nbare text line\nname: a\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
    assert_eq!(frontmatter.description, "d");
}

#[test]
fn parse_frontmatter_trimmed_leading_whitespace() {
    let frontmatter = fm("\n\n---\nname: a\ndescription: d\n---\n");
    assert_eq!(frontmatter.name.as_deref(), Some("a"));
}

#[test]
fn parse_frontmatter_quoted_values_not_unquoted() {
    let frontmatter = fm("---\ndescription: \"quoted\"\n---\n");
    assert_eq!(frontmatter.description, "\"quoted\"");
}

#[test]
fn dedup_prefers_project_scope() {
    let found = vec![
        skill("a", "same", "same", "/home/u/.codex/skills/a", true),
        skill("a", "same", "same", "/p/.claude/skills/a", false),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert!(!deduped[0].is_global);
    assert_eq!(deduped[0].path, PathBuf::from("/p/.claude/skills/a"));
}

#[test]
fn dedup_prefers_shorter_path_same_scope() {
    let found = vec![
        skill(
            "a",
            "same",
            "same",
            "/very/long/path/.claude/skills/a",
            true,
        ),
        skill("a", "same", "same", "/s/.claude/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].path, PathBuf::from("/s/.claude/skills/a"));
}

#[test]
fn dedup_keeps_first_on_length_tie() {
    let found = vec![
        skill("a", "same", "same", "/x/.claude/skills/a", true),
        skill("a", "same", "same", "/y/.claude/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].path, PathBuf::from("/x/.claude/skills/a"));
}

#[test]
fn dedup_renames_name_conflict_with_scope() {
    let found = vec![
        skill("a", "project desc", "p", "/p/.claude/skills/a", false),
        skill("a", "global desc", "g", "/h/.codex/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 2);
    assert_eq!(deduped[0].unique_name, ".global-a");
    assert_eq!(deduped[0].name, "a");
    assert_eq!(deduped[1].unique_name, "a");
    assert_eq!(deduped[1].name, "a");
}

#[test]
fn dedup_project_conflict_uses_project_scope() {
    let found = vec![
        skill("a", "project desc", "p", "/p/.claude/skills/a", false),
        skill("a", "other desc", "o", "/p/.codex/skills/a", false),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped[0].unique_name, ".project-a");
    assert_eq!(deduped[1].unique_name, "a");
}

#[test]
fn dedup_renames_with_numeric_counter() {
    let found = vec![
        skill("a", "project desc", "p", "/p/.claude/skills/a", false),
        skill("a", "global desc", "g", "/h/.codex/skills/a", true),
        skill(
            ".global-a",
            "taken",
            "t",
            "/h/.agents/skills/.global-a",
            true,
        ),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec![".global-.global-a", ".global-a", "a"]);
}

#[test]
fn dedup_renames_third_conflict_with_counter() {
    let found = vec![
        skill("a", "p1", "b1", "/p/.claude/skills/a", false),
        skill("a", "g1", "b1", "/h/.codex/skills/a", true),
        skill("a", "g2", "b2", "/h/.agents/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec![".global-a", ".global-a-2", "a"]);
}

#[test]
fn dedup_sorts_output_by_unique_name() {
    let found = vec![
        skill("b", "d", "b", "/p/.claude/skills/b", false),
        skill("a", "d", "a", "/p/.claude/skills/a", false),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn apply_enabled_defaults_and_overrides() {
    let mut skills = vec![
        skill("p", "d", "b", "/p/.claude/skills/p", false),
        skill("g", "d", "b", "/h/.codex/skills/g", true),
        skill("g2", "d", "b", "/h/.agents/skills/g2", true),
    ];
    let mut enabled = HashMap::new();
    enabled.insert("/p/.claude/skills/p".to_string(), false);
    enabled.insert("/h/.codex/skills/g".to_string(), true);
    apply_enabled(&mut skills, &enabled);
    assert!(!skills[0].is_enabled);
    assert!(skills[1].is_enabled);
    assert!(!skills[2].is_enabled);
}

#[test]
fn path_key_normalizes_backslashes() {
    assert_eq!(
        path_key(Path::new(r"C:\Users\alice\.claude\skills\a")),
        "C:/Users/alice/.claude/skills/a"
    );
    assert_eq!(
        path_key(Path::new("/home/alice/.claude/skills/a")),
        "/home/alice/.claude/skills/a"
    );
}

#[test]
fn read_enabled_map_missing_file_returns_empty() {
    let dir = temp_dir("missing-config");
    let map = read_enabled_map(&dir.join("config.toml")).expect("missing file is empty");
    assert!(map.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_parses_skill_table() {
    let dir = temp_dir("read-config");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[skill]\n\"/a/b\" = true\n\"/c/d\" = false\n").unwrap();
    let map = read_enabled_map(&path).expect("parse should succeed");
    assert_eq!(map.len(), 2);
    assert!(map["/a/b"]);
    assert!(!map["/c/d"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_non_bool_value_is_config_error() {
    let dir = temp_dir("non-bool");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[skill]\n\"/a/b\" = \"yes\"\n").unwrap();
    assert!(matches!(read_enabled_map(&path), Err(CoreError::Config(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_invalid_toml_is_config_error() {
    let dir = temp_dir("invalid-toml");
    let path = dir.join("config.toml");
    std::fs::write(&path, "not = [valid toml").unwrap();
    assert!(matches!(read_enabled_map(&path), Err(CoreError::Config(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_creates_parent_dirs_and_roundtrips() {
    let dir = temp_dir("write-config");
    let path = dir.join("nested").join("config.toml");
    let mut map = HashMap::new();
    map.insert("C:/Users/alice/.claude/skills/a".to_string(), false);
    write_enabled_map(&path, &map).expect("write should succeed");
    assert!(path.is_file());
    let read = read_enabled_map(&path).expect("read should succeed");
    assert_eq!(read, map);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_preserves_other_sections() {
    let dir = temp_dir("preserve-sections");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[other]\nfoo = 1\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["other"]["foo"], toml::Value::Integer(1));
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_missing_file_seeds_default_template() {
    let dir = temp_dir("seed-template");
    let path = dir.join("config.toml");
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# ManualAid 项目配置文件"));
    assert!(content.contains("[privacy_mask_extension.regex]"));
    assert!(content.contains("[privacy_mask_extension.literal]"));
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_blank_file_seeds_default_template() {
    let dir = temp_dir("seed-blank-template");
    let path = dir.join("config.toml");
    std::fs::write(&path, "  \n\t\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), false);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# ManualAid 项目配置文件"));
    assert_eq!(read_enabled_map(&path).expect("read should succeed"), map);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_preserves_comments_and_removes_stale_entries() {
    let dir = temp_dir("preserve-comments");
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "# 顶部注释\n[other]\nfoo = 1\n\n# 技能注释\n[skill]\n\"old\" = false\n",
    )
    .unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# 顶部注释"));
    assert!(content.contains("# 技能注释"));
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["other"]["foo"], toml::Value::Integer(1));
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    assert!(!table["skill"].as_table().unwrap().contains_key("old"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_fails_when_parent_is_a_file() {
    let dir = temp_dir("parent-file");
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    let err = write_enabled_map(&blocker.join("config.toml"), &map).expect_err("write should fail");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_when_path_is_a_directory_is_io_error() {
    let dir = temp_dir("read-dir-config");
    let err = read_enabled_map(&dir).expect_err("directory is not a readable config");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_when_path_is_a_directory_is_io_error() {
    let dir = temp_dir("write-dir-config");
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    let err = write_enabled_map(&dir, &map).expect_err("directory is not a writable config");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_without_parent_skips_dir_creation() {
    let path = Path::new("manualaid-bare-config-test.toml");
    let _ = std::fs::remove_file(path);
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), false);
    write_enabled_map(path, &map).expect("write should succeed");
    let read = read_enabled_map(path).expect("read should succeed");
    assert_eq!(read, map);
    let _ = std::fs::remove_file(path);
}

#[test]
fn write_enabled_map_replaces_non_table_skill_entry() {
    // A scalar `skill` key must be replaced by a table so the map can be
    // persisted; other keys in the document are untouched.
    // 标量 `skill` 键必须被替换为表才能持久化映射；文档中其余键不受影响。
    let dir = temp_dir("non-table-skill");
    let path = dir.join("config.toml");
    std::fs::write(&path, "skill = \"oops\"\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let table: Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn store_recovers_from_poisoned_locks() {
    // Poisoning the global store must not wedge later accesses: every guard
    // acquisition path recovers via `into_inner` because reloads rebuild
    // state from disk.
    // 让全局存储中毒后，后续访问不应被卡住：所有获取 guard 的路径都会
    // 通过 `into_inner` 恢复，因为重载总是从磁盘重建状态。
    let _ = std::panic::catch_unwind(|| {
        let _guard = STORE.write().unwrap();
        std::panic::panic_any("poison on purpose");
    });
    assert!(STORE.is_poisoned());
    assert!(read_store().project_root.is_none());
    let root = write_store().project_root.clone();
    assert!(root.is_none());
}
