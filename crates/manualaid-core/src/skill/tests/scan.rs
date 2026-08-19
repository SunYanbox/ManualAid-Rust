use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;

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
fn scan_skills_dir_rejects_file_path() {
    // `read_dir` on a file (rather than a directory) is a stable, portable
    // way to exercise the scanner's error branch. The `entry` iteration
    // error in `scan_skills_dir` still depends on a concurrent directory
    // change that tests cannot construct deterministically.
    // 对文件（而非目录）调用 `read_dir` 是稳定且可移植地覆盖扫描器错误
    // 分支的方式。`scan_skills_dir` 中条目遍历错误仍依赖测试无法确定性
    // 构造的并发目录变更。
    let dir = temp_dir("read-dir-file");
    let file = dir.join("not-a-dir");
    std::fs::write(&file, "content").unwrap();
    let err = scan_skills_dir(&file, false).expect_err("file path is not a directory");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn scan_skills_dir_skips_non_utf8_folder_names() {
    use std::os::unix::ffi::OsStringExt;

    // A folder whose name is not valid UTF-8 cannot become a skill name and
    // is skipped by the scanner.
    // 名称不是合法 UTF-8 的文件夹无法成为技能名，会被扫描器跳过。
    let dir = temp_dir("non-utf8-scan");
    let sub = dir.join(std::ffi::OsString::from_vec(b"bad\xffname".to_vec()));
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("SKILL.md"), "---\ndescription: d\n---\nbody").unwrap();
    let skills = scan_skills_dir(&dir, false).expect("scan should succeed");
    assert!(skills.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
