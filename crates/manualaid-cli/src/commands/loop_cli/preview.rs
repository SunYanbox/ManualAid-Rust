//! Approval previews: per-item context and colored diffs.
//! 审批预览：逐项上下文与着色 diff。

use indexmap::IndexMap;
use manualaid_core::audit::AuditQueueItem;
use serde_json::Value;

use super::utils::t_fmt;

/// The approval preview shown before each queue item.
/// 每个审批队列项展示前的预览文本。
pub fn approval_preview(item: &AuditQueueItem, params: &IndexMap<String, Value>) -> String {
    let header = t_fmt(
        "cli.approval.item",
        &[
            ("tool", &item.tool_name),
            ("param", &item.param_name),
            (
                "reason",
                item.decision.reason().unwrap_or("approval required"),
            ),
        ],
    );
    let mut detail = match item.tool_name.as_str() {
        "edit" => edit_diff_preview(params),
        "write" => write_preview(params),
        "shell" => params
            .get("command")
            .and_then(Value::as_str)
            .map(|command| format!("$ {command}"))
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
        detail.push_str(&t_fmt(
            "cli.approval.description",
            &[("description", description)],
        ));
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
            unified_diff(file_path, &original, &modified)
        }
        Err(_) => fallback(),
    }
}

/// Build a colored preview for a `write` approval: target info plus either
/// a capped diff against existing content or a capped content preview.
/// 为 `write` 审批构建预览：目标信息加上对已有内容的截断 diff，或
/// 不存在时的截断内容预览。
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
            let diff = unified_diff(file_path, &original, &content);
            if diff.is_empty() {
                out.push_str("\ncontent unchanged");
            } else {
                const MAX_DIFF_LINES: usize = 40;
                let lines: Vec<&str> = diff.lines().take(MAX_DIFF_LINES).collect();
                out.push('\n');
                out.push_str(&colorize_diff(&lines.join("\n")));
                if diff.lines().count() > MAX_DIFF_LINES {
                    out.push_str(&format!(
                        "\n... ({} more diff lines)",
                        diff.lines().count() - MAX_DIFF_LINES
                    ));
                }
            }
        }
        Err(_) => {
            if !content.is_empty() {
                const MAX_PREVIEW_LINES: usize = 40;
                let lines: Vec<&str> = content.lines().take(MAX_PREVIEW_LINES).collect();
                out.push('\n');
                out.push_str(&lines.join("\n"));
                if total > MAX_PREVIEW_LINES {
                    out.push_str(&format!("\n... ({} more lines)", total - MAX_PREVIEW_LINES));
                }
            }
        }
    }
    out
}

/// Produce a unified diff between two texts with `a/`/`b/` headers.
/// 生成两个文本之间带 `a/`/`b/` 头的 unified diff。
pub(super) fn unified_diff(path: &str, original: &str, modified: &str) -> String {
    similar::TextDiff::from_lines(original, modified)
        .unified_diff()
        .header(format!("a/{path}").as_str(), format!("b/{path}").as_str())
        .to_string()
}

/// Color a unified diff line-by-line, only when ANSI styling is enabled.
/// 逐行给 unified diff 着色；仅当 ANSI 样式启用时生效。
pub(super) fn colorize_diff(diff: &str) -> String {
    let mut out = String::new();
    for line in diff.lines() {
        let styled =
            if line.starts_with("@@") || line.starts_with("--- ") || line.starts_with("+++ ") {
                crate::style::cyan(line)
            } else if line.starts_with('-') {
                crate::style::red(line)
            } else if line.starts_with('+') {
                crate::style::green(line)
            } else if line.starts_with(' ') {
                crate::style::gray(line)
            } else {
                line.to_string()
            };
        out.push_str(&styled);
        out.push('\n');
    }
    out
}
