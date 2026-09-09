use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;

#[test]
fn dedup_prefers_project_scope() {
    let found = vec![
        skill(
            "a",
            "same",
            "same",
            ".codex",
            "/home/u/.codex/skills/a",
            true,
        ),
        skill("a", "same", "same", ".claude", "/p/.claude/skills/a", false),
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
            ".claude",
            "/very/long/path/.claude/skills/a",
            true,
        ),
        skill("a", "same", "same", ".claude", "/s/.claude/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].path, PathBuf::from("/s/.claude/skills/a"));
}

#[test]
fn dedup_keeps_first_on_length_tie() {
    let found = vec![
        skill("a", "same", "same", ".claude", "/x/.claude/skills/a", true),
        skill("a", "same", "same", ".claude", "/y/.claude/skills/a", true),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].path, PathBuf::from("/x/.claude/skills/a"));
}

#[test]
fn dedup_assigns_stable_names_across_scopes() {
    let found = vec![
        skill(
            "a",
            "project desc",
            "p",
            ".claude",
            "/p/.claude/skills/a",
            false,
        ),
        skill(
            "a",
            "global desc",
            "g",
            ".codex",
            "/h/.codex/skills/a",
            true,
        ),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<(&str, &str)> = deduped
        .iter()
        .map(|s| (s.unique_name.as_str(), s.name.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![("global-.codex-a", "a"), ("project-.claude-a", "a")]
    );
}

#[test]
fn dedup_distinguishes_same_name_in_other_dirs() {
    let found = vec![
        skill(
            "a",
            "claude desc",
            "p",
            ".claude",
            "/p/.claude/skills/a",
            false,
        ),
        skill(
            "a",
            "codex desc",
            "o",
            ".codex",
            "/p/.codex/skills/a",
            false,
        ),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec!["project-.claude-a", "project-.codex-a"]);
}

#[test]
fn dedup_suffixes_same_root_name_collision() {
    // Two folders under the same root declare the same frontmatter `name`
    // with different content: the scanner assigns the same stable name, and
    // deduplication appends a deterministic `-N` suffix to the later copy.
    // 同一根目录下两个文件夹以不同内容声明了相同的 frontmatter `name`：
    // 扫描器赋予相同的稳定名，去重为后一个副本追加确定性 `-N` 后缀。
    let found = vec![
        skill("a", "first", "p", ".agents", "/p/.agents/skills/a", false),
        skill(
            "a",
            "later",
            "g",
            ".agents",
            "/p/.agents/skills/a-copy",
            false,
        ),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec!["project-.agents-a", "project-.agents-a-2"]);
}

#[test]
fn dedup_counter_skips_other_natural_names() {
    // The suffixed candidate must not shadow a real skill whose name is
    // `pdf-2`: it is reserved for that skill and the duplicate moves on.
    // 后缀候选不得遮蔽名为 `pdf-2` 的真实技能：该名称为其保留，重复技能
    // 继续取下一个后缀。
    let found = vec![
        skill(
            "pdf",
            "original",
            "b1",
            ".agents",
            "/p/.agents/skills/pdf",
            false,
        ),
        skill(
            "pdf-2",
            "real",
            "b2",
            ".agents",
            "/p/.agents/skills/pdf-2",
            false,
        ),
        skill(
            "pdf",
            "copy",
            "b3",
            ".agents",
            "/p/.agents/skills/pdf-copy",
            false,
        ),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "project-.agents-pdf",
            "project-.agents-pdf-2",
            "project-.agents-pdf-3"
        ]
    );
}

#[test]
fn dedup_sorts_output_by_unique_name() {
    let found = vec![
        skill("b", "d", "b", ".claude", "/p/.claude/skills/b", false),
        skill("a", "d", "a", ".claude", "/p/.claude/skills/a", false),
    ];
    let deduped = dedup_skills(found);
    let names: Vec<&str> = deduped.iter().map(|s| s.unique_name.as_str()).collect();
    assert_eq!(names, vec!["project-.claude-a", "project-.claude-b"]);
}

#[test]
fn dedup_treats_crlf_and_whitespace_as_equal() {
    let found = vec![
        skill(
            "a",
            "  says hi\n",
            "line1\nline2",
            ".claude",
            "/p/.claude/skills/a",
            false,
        ),
        skill(
            "a",
            "says hi",
            "line1\r\nline2 ",
            ".codex",
            "/h/.codex/skills/a",
            true,
        ),
    ];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert!(!deduped[0].is_global);
}

#[test]
fn dedup_does_not_rewrite_stored_text() {
    let found = vec![skill(
        "a",
        "  padded desc  ",
        "line1\r\nline2",
        ".claude",
        "/p/.claude/skills/a",
        false,
    )];
    let deduped = dedup_skills(found);
    assert_eq!(deduped.len(), 1);
    assert_eq!(deduped[0].description, "  padded desc  ");
    assert_eq!(deduped[0].body, "line1\r\nline2");
}

#[test]
fn stable_unique_name_composes_scope_dir_and_name() {
    assert_eq!(
        stable_unique_name(false, ".claude", "pdf"),
        "project-.claude-pdf"
    );
    assert_eq!(
        stable_unique_name(true, ".agents", "find-skills"),
        "global-.agents-find-skills"
    );
}

#[test]
fn apply_enabled_defaults_and_overrides() {
    let mut skills = vec![
        skill("p", "d", "b", ".claude", "/p/.claude/skills/p", false),
        skill("g", "d", "b", ".codex", "/h/.codex/skills/g", true),
        skill("g2", "d", "b", ".agents", "/h/.agents/skills/g2", true),
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
fn scan_skills_dir_records_agent_dir_and_stable_name() {
    // The scanner stamps the agent directory and the stable unique name on
    // every skill, and applies the global-scope enabled default.
    // 扫描器在每个技能上记录 agent 目录与稳定唯一名称，并应用全局范围的
    // 启用默认值。
    let dir = temp_dir("scan-stable-name");
    let skill_dir = dir.join(".agents").join("skills").join("pdf");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: pdf\ndescription: read pdf\n---\nbody",
    )
    .unwrap();
    let skills = scan_skills_dir(&dir.join(".agents").join("skills"), ".agents", true)
        .expect("scan should succeed");
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].unique_name, "global-.agents-pdf");
    assert_eq!(skills[0].agent_dir, ".agents");
    assert!(!skills[0].is_enabled);
    let _ = std::fs::remove_dir_all(&dir);
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
    let err = scan_skills_dir(&file, ".claude", false).expect_err("file path is not a directory");
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
    let skills = scan_skills_dir(&dir, ".claude", false).expect("scan should succeed");
    assert!(skills.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
