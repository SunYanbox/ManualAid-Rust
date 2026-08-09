//! Approval previews: per-item context and colored diffs.
//! 审批预览：逐项上下文与着色 diff。

use indexmap::IndexMap;
use manualaid_core::audit::AuditQueueItem;
use serde_json::Value;

use super::diff::{colored_diff, colorize_diff};
use super::utils::t_fmt;

/// The approval preview shown before each queue item.
/// 每个审批队列项展示前的预览文本。
pub fn approval_preview(item: &AuditQueueItem, params: &IndexMap<String, Value>) -> String {
    // The approval line is metadata; the command and the AI-supplied purpose
    // below are what the user actually judges the operation on.
    // 审批行只是元信息，真正用于判断操作的是下方的命令与 AI 提供的目的。
    let header = crate::style::muted(&t_fmt(
        "cli.approval.item",
        &[
            ("tool", &item.tool_name),
            ("param", &item.param_name),
            (
                "reason",
                item.decision.reason().unwrap_or("approval required"),
            ),
        ],
    ));
    let mut detail = match item.tool_name.as_str() {
        "edit" => edit_diff_preview(params),
        "write" => write_preview(params),
        "shell" => params
            .get("command")
            .and_then(Value::as_str)
            .map(|command| crate::style::accent(&format!("$ {command}")))
            .unwrap_or_default(),
        _ => params
            .get(&item.param_name)
            .map(Value::to_string)
            .unwrap_or_default(),
    };
    // Show the AI-supplied purpose (`description`) so the user can judge the
    // operation without expanding the raw command.
    // 展示 AI 提供的调用目的（`description`），让用户无需展开原始命令即可
    // 判断操作。
    if let Some(description) = params.get("description").and_then(Value::as_str)
        && !description.trim().is_empty()
    {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&crate::style::yellow(&t_fmt(
            "cli.approval.description",
            &[("description", description)],
        )));
    }
    if detail.trim().is_empty() {
        header
    } else {
        format!("{header}\n{detail}")
    }
}

/// Build a colored unified diff for an `edit` approval preview, falling
/// back to a `-`/`+` block when the file cannot be read or the replacement
/// would not change anything.
/// 为 `edit` 审批预览构建彩色 unified diff；文件不可读或替换不产生变化
/// 时回退为 `-`/`+` 块。
pub(super) fn edit_diff_preview(params: &IndexMap<String, Value>) -> String {
    let (Some(file_path), Some(old_string), Some(new_string)) = (
        params.get("file_path").and_then(Value::as_str),
        params.get("old_string").and_then(Value::as_str),
        params.get("new_string").and_then(Value::as_str),
    ) else {
        return String::new();
    };
    let replace_all = params
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let fallback = || colorize_diff(&format!("- {old_string}\n+ {new_string}"));
    match std::fs::read_to_string(file_path) {
        Ok(original) => {
            let modified = if replace_all {
                original.replace(old_string, new_string)
            } else {
                original.replacen(old_string, new_string, 1)
            };
            if modified == original {
                return fallback();
            }
            colored_diff(file_path, &original, &modified)
        }
        Err(_) => fallback(),
    }
}

/// Build a colored preview for a `write` approval: target info plus either
/// a full diff against existing content or the full content preview. Long
/// previews are paged by the caller, so nothing is truncated here.
/// 为 `write` 审批构建预览：目标信息加上对已有内容的完整 diff，或不存在时
/// 的完整内容预览。长预览由调用方分页，这里不做截断。
pub(super) fn write_preview(params: &IndexMap<String, Value>) -> String {
    let Some(file_path) = params.get("file_path").and_then(Value::as_str) else {
        return String::new();
    };
    let content = match params.get("content") {
        Some(Value::String(content)) => content.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    let total = content.lines().count();
    let mut out = format!("write {file_path} ({total} lines, {} bytes)", content.len());
    match std::fs::read_to_string(file_path) {
        Ok(original) => {
            let diff = colored_diff(file_path, &original, &content);
            if diff.is_empty() {
                out.push_str("\ncontent unchanged");
            } else {
                out.push('\n');
                out.push_str(&diff);
            }
        }
        Err(_) => {
            if !content.is_empty() {
                out.push('\n');
                out.push_str(&content);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use manualaid_core::audit::{AuditDecision, AuditQueueItem};

    fn params(pairs: &[(&str, &str)]) -> IndexMap<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
            .collect()
    }

    fn queue_item(tool: &str, param: &str, reason: Option<&str>) -> AuditQueueItem {
        AuditQueueItem {
            tool_name: tool.to_string(),
            param_name: param.to_string(),
            decision: reason
                .map(|reason| AuditDecision::NeedsApproval(reason.to_string()))
                .unwrap_or(AuditDecision::Allowed),
        }
    }

    #[test]
    fn edit_preview_shows_diff_against_existing_file() {
        let root = crate::test_support::temp_dir("edit-preview");
        let file = root.join("doc.txt");
        std::fs::write(&file, "old text\n").unwrap();
        let preview = edit_diff_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("old_string", "old"),
            ("new_string", "new"),
        ]));
        assert!(preview.contains("-old text"));
        assert!(preview.contains("+new text"));
    }

    #[test]
    fn edit_preview_replace_all_swaps_every_occurrence() {
        let root = crate::test_support::temp_dir("edit-replace-all");
        let file = root.join("doc.txt");
        std::fs::write(&file, "x a x b x\n").unwrap();
        let mut preview = edit_diff_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("old_string", "x"),
            ("new_string", "y"),
        ]));
        assert!(preview.contains("-x a x b x"));
        assert!(preview.contains("+y a x b x"));
        let mut replace_all = params(&[
            ("file_path", file.to_str().unwrap()),
            ("old_string", "x"),
            ("new_string", "y"),
        ]);
        replace_all.insert("replace_all".to_string(), Value::Bool(true));
        preview = edit_diff_preview(&replace_all);
        assert!(preview.contains("-x a x b x"));
        assert!(preview.contains("+y a y b y"));
    }

    #[test]
    fn edit_preview_missing_file_falls_back_to_block() {
        let preview = edit_diff_preview(&params(&[
            ("file_path", "Z:/missing/preview.txt"),
            ("old_string", "old"),
            ("new_string", "new"),
        ]));
        assert!(preview.contains("- old"));
        assert!(preview.contains("+ new"));
    }

    #[test]
    fn edit_preview_unchanged_content_falls_back_to_block() {
        let root = crate::test_support::temp_dir("edit-unchanged");
        let file = root.join("doc.txt");
        std::fs::write(&file, "same\n").unwrap();
        let preview = edit_diff_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("old_string", "same"),
            ("new_string", "same"),
        ]));
        assert!(preview.contains("- same"));
        assert!(preview.contains("+ same"));
    }

    #[test]
    fn edit_preview_missing_params_is_empty() {
        assert!(edit_diff_preview(&IndexMap::new()).is_empty());
    }

    #[test]
    fn write_preview_shows_diff_against_existing_file() {
        let root = crate::test_support::temp_dir("write-preview");
        let file = root.join("doc.txt");
        std::fs::write(&file, "old\n").unwrap();
        let preview = write_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("content", "new\n"),
        ]));
        assert!(preview.contains(&format!("write {} (1 lines", file.display())));
        assert!(preview.contains("-old"));
        assert!(preview.contains("+new"));
    }

    #[test]
    fn write_preview_unchanged_content_is_marked() {
        let root = crate::test_support::temp_dir("write-unchanged");
        let file = root.join("doc.txt");
        std::fs::write(&file, "same\n").unwrap();
        let preview = write_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("content", "same\n"),
        ]));
        assert!(preview.contains("content unchanged"));
    }

    #[test]
    fn write_preview_missing_file_shows_content() {
        let preview = write_preview(&params(&[
            ("file_path", "Z:/missing/write.txt"),
            ("content", "line one\nline two\n"),
        ]));
        assert!(preview.contains("line one"));
        assert!(preview.contains("line two"));
    }

    #[test]
    fn write_preview_shows_full_content_for_new_files() {
        let content = (1..=50)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let preview = write_preview(&params(&[
            ("file_path", "Z:/missing/long.txt"),
            ("content", &content),
        ]));
        assert!(preview.contains("line 1"));
        assert!(preview.contains("line 50"));
        assert!(!preview.contains("more lines"));
    }

    #[test]
    fn write_preview_shows_full_diff_without_truncation() {
        let root = crate::test_support::temp_dir("write-long-diff");
        let file = root.join("doc.txt");
        let original = (1..=50)
            .map(|n| format!("old {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let content = (1..=50)
            .map(|n| format!("new {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&file, &original).unwrap();
        let preview = write_preview(&params(&[
            ("file_path", file.to_str().unwrap()),
            ("content", &content),
        ]));
        assert!(preview.contains("old 1"));
        assert!(preview.contains("new 50"));
        assert!(!preview.contains("more diff lines"));
    }

    #[test]
    fn write_preview_missing_path_is_empty() {
        assert!(write_preview(&IndexMap::new()).is_empty());
    }

    #[test]
    fn write_preview_accepts_non_string_content() {
        let preview = write_preview(&params(&[("file_path", "Z:/missing/n.txt")]));
        assert!(preview.starts_with("write Z:/missing/n.txt"));
        let mut numeric = params(&[("file_path", "Z:/missing/n.txt")]);
        numeric.insert("content".to_string(), Value::Number(42.into()));
        let preview = write_preview(&numeric);
        assert!(preview.contains("42"));
    }

    #[test]
    fn approval_preview_edit_tool_shows_diff() {
        let root = crate::test_support::temp_dir("approval-edit");
        let file = root.join("doc.txt");
        std::fs::write(&file, "old\n").unwrap();
        let item = queue_item("edit", "old_string", Some("outside"));
        let preview = approval_preview(
            &item,
            &params(&[
                ("file_path", file.to_str().unwrap()),
                ("old_string", "old"),
                ("new_string", "new"),
            ]),
        );
        assert!(preview.contains("+new"));
    }

    #[test]
    fn approval_preview_write_tool_shows_target() {
        let item = queue_item("write", "file_path", Some("outside"));
        let preview = approval_preview(
            &item,
            &params(&[
                ("file_path", "Z:/missing/approval.txt"),
                ("content", "hello"),
            ]),
        );
        assert!(preview.contains("write Z:/missing/approval.txt"));
        assert!(preview.contains("hello"));
    }

    #[test]
    fn approval_preview_shell_without_command_uses_header_only() {
        let item = queue_item("shell", "command", Some("needs review"));
        let preview = approval_preview(&item, &IndexMap::new());
        assert!(!preview.contains("$ "));
        assert!(preview.contains("needs review"));
    }

    #[test]
    fn approval_preview_empty_detail_uses_header_only() {
        let item = queue_item("read", "missing_param", Some("outside"));
        let preview = approval_preview(&item, &IndexMap::new());
        assert!(preview.contains("outside"));
        assert!(!preview.contains('\n'));
    }

    #[test]
    fn approval_preview_uses_default_reason_without_decision_reason() {
        let item = queue_item("read", "file_path", None);
        let preview = approval_preview(&item, &params(&[("file_path", "Z:/a.txt")]));
        assert!(preview.contains("approval required"));
    }

    #[test]
    fn approval_preview_appends_description() {
        let item = queue_item("read", "file_path", Some("outside"));
        let preview = approval_preview(
            &item,
            &params(&[("file_path", "Z:/a.txt"), ("description", "check the file")]),
        );
        assert!(preview.contains("check the file"));
    }
}
