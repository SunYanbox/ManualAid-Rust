//! Integration tests for the unified tool layer: definitions, routing and
//! async execution of every built-in tool.
//! 统一工具层的集成测试：定义、路由与每个内置工具的异步执行。

use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;
use manualaid_core::tools::{ToolCallFormat, ToolKind, ToolResult, all_tools, params_summary_of};
use serde_json::Value;

/// A unique temporary file path (not pre-created).
/// 唯一临时文件路径（不预先创建）。
fn temp_file(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-core-tools-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn all_tools_are_stable_and_unique() {
    let tools = all_tools();
    assert_eq!(tools.len(), 5);
    let names: Vec<&str> = tools.iter().map(ToolKind::name).collect();
    let unique: std::collections::HashSet<_> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len());
}

#[test]
fn from_name_round_trips_every_tool() {
    for tool in all_tools() {
        assert_eq!(ToolKind::from_name(tool.name()), Some(*tool));
    }
    assert_eq!(ToolKind::from_name("nope"), None);
}

#[test]
fn all_formats_lists_every_builtin_variant() {
    assert_eq!(
        ToolCallFormat::all(),
        &[ToolCallFormat::Xml, ToolCallFormat::JsonCodeblock]
    );
}

#[test]
fn parameters_carry_semantic_tags_and_i18n_keys() {
    for tool in all_tools() {
        for param in tool.parameters() {
            assert!(!param.description_key.is_empty());
            assert!(!param.description().is_empty());
        }
    }
    let write_param = ToolKind::Edit
        .parameters()
        .into_iter()
        .find(|p| p.name == "file_path")
        .expect("edit has file_path");
    assert!(write_param.semantic.is_write());
}

#[test]
fn read_only_flags_match_tool_kind() {
    assert!(ToolKind::Read.is_read_only());
    assert!(ToolKind::Skill.is_read_only());
    assert!(!ToolKind::Edit.is_read_only());
    assert!(!ToolKind::Write.is_read_only());
    assert!(!ToolKind::Shell.is_read_only());
}

#[tokio::test]
async fn write_then_read_round_trip() {
    let path = temp_file("roundtrip");
    let write_params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("content", "hello\nworld"),
    ]);
    let result = ToolKind::Write.run(&write_params).await;
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("bytes"));

    let read_params = params_for(&[("file_path", path.to_str().unwrap())]);
    let result = ToolKind::Read.run(&read_params).await;
    assert!(result.success);
    assert_eq!(result.output, "hello\nworld");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn read_supports_offset_and_limit() {
    let path = temp_file("slice");
    std::fs::write(&path, "a\nb\nc\nd\n").unwrap();
    let mut params = IndexMap::new();
    params.insert(
        "file_path".to_string(),
        Value::String(path.to_str().unwrap().to_string()),
    );
    params.insert("offset".to_string(), Value::from(2));
    params.insert("limit".to_string(), Value::from(2));
    let result = ToolKind::Read.run(&params).await;
    assert!(result.success);
    assert_eq!(result.output, "b\nc\n");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn read_missing_file_fails() {
    let params = params_for(&[("file_path", "Z:/definitely/missing.txt")]);
    let result = ToolKind::Read.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("read"));
}

#[tokio::test]
async fn edit_replaces_first_occurrence_and_rejects_ambiguous() {
    let path = temp_file("edit");
    std::fs::write(&path, "one two one").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "one"),
        ("new_string", "ONE"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success, "ambiguous old_string must fail");
    assert!(result.output.contains("appears 2 times"));

    let mut params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "one"),
        ("new_string", "ONE"),
    ]);
    params.insert("replace_all".to_string(), Value::Bool(true));
    let result = ToolKind::Edit.run(&params).await;
    assert!(result.success, "{}", result.output);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "ONE two ONE");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_single_replace_uses_first_occurrence() {
    let path = temp_file("edit-single");
    std::fs::write(&path, "ab a").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "b"),
        ("new_string", "X"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("Replaced 1 occurrence"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "aX a");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_reports_negative_line_delta() {
    let path = temp_file("edit-negative-delta");
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "a\nb"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("(3 -> 2 lines, -1 lines)"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "x\nc\n");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_diff_shows_context_and_insertion() {
    let path = temp_file("edit-context-diff");
    std::fs::write(&path, "a\nb\nc\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "a\nb\nc"),
        ("new_string", "a\nB\nc"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(result.success, "{}", result.output);
    // unified diff 中上下文行以空格开头、删除行以 - 开头、新增行以 + 开头
    assert!(result.output.contains("```diff\n  a\n- b\n+ B\n  c\n```"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nB\nc\n");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_diff_is_truncated_beyond_200_lines() {
    let path = temp_file("edit-truncated-diff");
    let old_lines: Vec<String> = (0..120).map(|i| format!("line {i}")).collect();
    let new_lines: Vec<String> = (0..120).map(|i| format!("other {i}")).collect();
    let old = old_lines.join("\n");
    let new = new_lines.join("\n");
    std::fs::write(&path, format!("{old}\n")).unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", &old),
        ("new_string", &new),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(result.success, "{}", result.output);
    // 120 行删除 + 120 行新增 = 240 行 diff，超过 200 行上限被截断
    assert!(result.output.contains("(40 more lines truncated)"));
    assert!(result.output.contains("+ other 79"));
    assert!(!result.output.contains("+ other 119"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_reports_missing_required_params() {
    let result = ToolKind::Edit.run(&IndexMap::new()).await;
    assert!(!result.success);
    assert!(result.output.contains("file_path"));

    let params = params_for(&[("file_path", "/tmp/x.txt")]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("old_string"));
}

#[cfg(windows)]
#[tokio::test]
async fn edit_reports_write_failure() {
    let path = temp_file("edit-readonly");
    std::fs::write(&path, "old").unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "old"),
        ("new_string", "new"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("cannot write file"));
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    // Only restores writability so the temp file can be removed.
    // 仅用于恢复可写属性，以便删除临时文件。
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_rejects_missing_old_string() {
    let path = temp_file("edit-missing");
    std::fs::write(&path, "content").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "absent"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_missing_old_string_suggests_closest_match() {
    let path = temp_file("edit-closest");
    std::fs::write(&path, "line one\nline two\nline three\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "line one\nline twO"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
    assert!(result.output.contains("Closest match"));
    assert!(result.output.contains("line one\nline two"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_missing_old_string_no_similar_suggestion() {
    let path = temp_file("edit-no-closest");
    std::fs::write(&path, "foo bar\nbaz\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "xyzzy"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
    assert!(!result.output.contains("Closest match"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_missing_old_string_crlf_only_diff() {
    let path = temp_file("edit-crlf-file");
    std::fs::write(&path, "a\r\nb\r\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "a\nb"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("line endings differ"));
    assert!(result.output.contains("CRLF"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_missing_old_string_crlf_reverse_only_diff() {
    let path = temp_file("edit-crlf-old");
    std::fs::write(&path, "a\nb\n").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "a\r\nb"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("line endings differ"));
    assert!(result.output.contains("`old_string` uses CRLF"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn edit_missing_old_string_mixed_line_endings() {
    let path = temp_file("edit-mixed-eol");
    std::fs::write(&path, "a\r\nb\nc").unwrap();
    let params = params_for(&[
        ("file_path", path.to_str().unwrap()),
        ("old_string", "b\r\nc"),
        ("new_string", "x"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("line endings differ"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn write_creates_parent_directories() {
    let dir = std::env::temp_dir().join(format!("manualaid-core-tools-dir-{}", std::process::id()));
    let path = dir.join("nested").join("file.txt");
    let params = params_for(&[("file_path", path.to_str().unwrap()), ("content", "data")]);
    let result = ToolKind::Write.run(&params).await;
    assert!(result.success);
    assert!(path.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn write_to_directory_path_fails() {
    let dir = std::env::temp_dir().join(format!("manualaid-core-write-dir-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let params = params_for(&[("file_path", dir.to_str().unwrap()), ("content", "x")]);
    let result = ToolKind::Write.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("cannot write file"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn write_reports_missing_required_params() {
    let result = ToolKind::Write.run(&IndexMap::new()).await;
    assert!(!result.success);
    assert!(result.output.contains("file_path"));

    let params = params_for(&[("file_path", "/tmp/x.txt")]);
    let result = ToolKind::Write.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("content"));
}

#[tokio::test]
async fn edit_reports_invalid_params() {
    let params = params_for(&[
        ("file_path", "/tmp/x.txt"),
        ("old_string", ""),
        ("new_string", "y"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("must not be empty"));

    let params = params_for(&[
        ("file_path", "/tmp/x.txt"),
        ("old_string", "a"),
        ("new_string", "a"),
    ]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("must be different"));

    // new_string 为空时表示删除，是合法操作，不再报错。
    // 但 old_string 为空仍然应当报错。
    let params = params_for(&[("file_path", "/tmp/x.txt"), ("old_string", "")]);
    let result = ToolKind::Edit.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("old_string"));
}

#[tokio::test]
async fn read_rejects_invalid_slice_params() {
    let path = temp_file("slice-invalid");
    std::fs::write(&path, "a\n").unwrap();
    let mut params = IndexMap::new();
    params.insert(
        "file_path".to_string(),
        Value::String(path.to_str().unwrap().to_string()),
    );
    params.insert("offset".to_string(), Value::from(-1));
    let result = ToolKind::Read.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("offset"));

    let mut params = IndexMap::new();
    params.insert(
        "file_path".to_string(),
        Value::String(path.to_str().unwrap().to_string()),
    );
    params.insert("offset".to_string(), Value::from(9));
    let result = ToolKind::Read.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("exceeds total line count"));

    let mut params = IndexMap::new();
    params.insert(
        "file_path".to_string(),
        Value::String(path.to_str().unwrap().to_string()),
    );
    params.insert("limit".to_string(), Value::from(-3));
    let result = ToolKind::Read.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("limit"));
    let _ = std::fs::remove_file(&path);
}

#[cfg(windows)]
#[tokio::test]
async fn shell_timeout_aborts_long_command() {
    let mut params = params_for(&[("command", "ping 127.0.0.1 -n 3"), ("description", "slow")]);
    params.insert("timeout".to_string(), Value::from(50));
    let result = ToolKind::Shell.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("timed out"));
}

#[tokio::test]
async fn shell_captures_output_and_exit_code() {
    let result = ToolKind::Shell
        .run(&params_for(&[
            ("command", "echo manualaid"),
            ("description", "test"),
        ]))
        .await;
    assert!(result.success, "{}", result.output);
    assert!(result.output.contains("manualaid"));

    let result = ToolKind::Shell
        .run(&params_for(&[
            ("command", "exit 3"),
            ("description", "test"),
        ]))
        .await;
    assert!(!result.success);
    assert!(result.output.contains("exited with code"));
}

#[tokio::test]
async fn shell_requires_command_param() {
    let result = ToolKind::Shell.run(&IndexMap::new()).await;
    assert!(!result.success);
    assert!(result.output.contains("command"));
}

#[tokio::test]
async fn skill_tool_reports_unknown_skill() {
    let params = params_for(&[("skill", "definitely-not-a-skill")]);
    let result = ToolKind::Skill.run(&params).await;
    assert!(!result.success);
    assert!(result.output.contains("not found"));
}

#[test]
fn params_summary_is_truncated_json() {
    let mut params = IndexMap::new();
    params.insert("content".to_string(), Value::String("x".repeat(200)));
    let summary = params_summary_of(&params);
    assert!(summary.starts_with('{'));
    assert!(summary.chars().count() <= 75);
}

#[test]
fn tool_result_constructors_set_flags() {
    let ok = ToolResult::success("read", "out", true);
    assert!(ok.success && ok.read_only && !ok.is_fallback);
    let err = ToolResult::failure("edit", "msg");
    assert!(!err.success && err.is_fallback);
}

/// Build an ordered parameter map from string pairs.
/// 从字符串对构建有序参数映射。
fn params_for(pairs: &[(&str, &str)]) -> IndexMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
        .collect()
}

#[tokio::test]
async fn read_without_file_path_is_a_failure() {
    let result = ToolKind::Read.run(&IndexMap::new()).await;
    assert!(!result.success);
    assert!(result.output.contains("file_path"));
}
