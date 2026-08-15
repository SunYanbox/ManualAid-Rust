//! Edit tool execution: exact string replacement with pre-verification.
//! The pre-verify logic in `plan_edit` is shared with the executor's
//! pre-check so calls guaranteed to fail never reach the approval queue.
//! `plan_edit` and [`EditPlan`] are also public so the CLI debug tools
//! exercise the same validation path as real execution.
//! Edit 工具执行：精确字符串替换并预验证。`plan_edit` 中的预验证逻辑与
//! 执行器的预检共用，保证必然失败的调用不会进入批准队列。`plan_edit`
//! 与 [`EditPlan`] 对外公开，使 CLI 调试工具走与真实执行相同的校验路径。

use indexmap::IndexMap;
use serde_json::Value;

use super::{ToolResult, get_bool, get_string};
use crate::async_fs::{read_file, write_file};

/// The validated plan of one edit call: file content and the number of
/// `old_string` occurrences captured before any modification.
/// 一次 edit 调用的已验证计划：修改前捕获的文件内容与 `old_string`
/// 出现次数。
///
/// # Description
/// Public because the `debug plan_edit` CLI command reports the pre-verify
/// outcome without modifying the file; `content` is the original text the
/// search ran against, which debug output needs.
/// # 描述
/// 对外公开，因为 `debug plan_edit` CLI 命令只报告预检结果而不修改文件；
/// `content` 是搜索所针对的原文本，调试输出需要用到。
pub struct EditPlan {
    /// Target file path.
    /// 目标文件路径。
    pub file_path: String,
    /// Text to search for.
    /// 要查找的文本。
    pub old_string: String,
    /// Replacement text.
    /// 替换文本。
    pub new_string: String,
    /// Whether to replace every occurrence.
    /// 是否替换全部出现。
    pub replace_all: bool,
    /// Current file content.
    /// 文件当前内容。
    pub content: String,
    /// Number of `old_string` occurrences in the current content (>= 1).
    /// `old_string` 在当前内容中的出现次数（至少为 1）。
    pub count: usize,
}

/// Extract and pre-verify one edit call, returning an [`EditPlan`].
/// Any condition that would make the edit fail (missing or ambiguous
/// `old_string`, unreadable file, ...) is reported as `Err(message)`.
/// 提取并预验证一次 edit 调用，返回 [`EditPlan`]。
/// 任何必然导致编辑失败的条件（`old_string` 缺失或重复、文件不可读等）
/// 都以 `Err(message)` 返回。
/// When `old_string` is not found, the error additionally points out a
/// line-ending mismatch or suggests the closest highly similar string.
/// 当未找到 `old_string` 时，错误消息还会提示换行符差异或建议最相似的字符串。
///
/// # Description
/// Public so external callers (the CLI debug tools) can validate against the
/// real execution path; it never writes to the file. The parameter map is
/// the same wire format the tool executor consumes.
/// # 描述
/// 对外公开，使外部调用方（CLI 调试工具）能走真实执行路径做校验；本函数
/// 不会写入文件。参数映射与工具执行器消费的线格式一致。
pub async fn plan_edit(params: &IndexMap<String, Value>) -> Result<EditPlan, String> {
    let file_path = get_string(params, "file_path")
        .ok_or_else(|| "Missing required parameter `file_path`".to_string())?;
    let old_string = get_string(params, "old_string")
        .ok_or_else(|| "Missing required parameter `old_string`".to_string())?;
    if old_string.is_empty() {
        return Err("`old_string` must not be empty".to_string());
    }
    let new_string = get_string(params, "new_string").unwrap_or_default();
    let replace_all = get_bool(params, "replace_all").unwrap_or(false);

    if old_string == new_string {
        return Err("`old_string` and `new_string` must be different".to_string());
    }

    let content = read_file(&file_path).await.map_err(|e| e.to_string())?;

    if !content.contains(&old_string) {
        return Err(missing_old_string_message(
            &file_path,
            &old_string,
            &content,
        ));
    }

    let count = content.matches(&old_string).count();
    if !replace_all && count > 1 {
        return Err(format!(
            "`old_string` appears {count} times in `{file_path}` — use `replace_all: true` \
             to replace all, or make the pattern more specific"
        ));
    }

    Ok(EditPlan {
        file_path,
        old_string,
        new_string,
        replace_all,
        content,
        count,
    })
}

/// Execute one edit parameter set.
/// 执行一组 edit 参数。
/// Builds the not-found error message and appends diagnostic suggestions.
/// 构造 `old_string` 未找到的错误消息并附加诊断建议。
///
/// # Description
/// Keeps the original failure text intact and adds one of two diagnostics:
/// an explicit line-ending mismatch note, or the closest highly similar
/// string from the file.
/// # 描述
/// 保留原有失败文本，并补充两类诊断之一：明确的换行符差异提示，或文件中
/// 高度相似的最接近字符串。
fn missing_old_string_message(file_path: &str, old_string: &str, content: &str) -> String {
    let base = format!(
        "`old_string` not found in `{file_path}` — it may have already been applied \
         or the content has changed. Use the `read` tool to re-read `{file_path}` \
         and re-issue the edit with the current content.\n\
         String:\n{old_string}"
    );

    if let Some(note) = line_ending_mismatch(content, old_string) {
        return format!("{base}\n{note}");
    }

    if let Some(candidate) = closest_match(content, old_string) {
        return format!("{base}\nClosest match in the file (similarity >= 90%):\n{candidate}");
    }

    base
}

/// Detects whether the only difference between `content` and `old` is line
/// ending style, returning a note describing which side uses CRLF vs LF.
/// 检测 `content` 与 `old` 是否仅换行风格不同，返回描述哪一侧使用
/// CRLF/LF 的提示。
///
/// # Description
/// Only used after the raw `contains` check failed, so a hit here means the
/// normalized forms are equal.
/// # 描述
/// 仅在原始 `contains` 检查失败后调用，因此命中即表示规范化后完全一致。
fn line_ending_mismatch(content: &str, old: &str) -> Option<String> {
    let content_normalized = content.replace("\r\n", "\n");
    let old_normalized = old.replace("\r\n", "\n");

    if !content_normalized.contains(&old_normalized) {
        return None;
    }

    let content_crlf = content.contains("\r\n");
    let old_crlf = old.contains("\r\n");
    let content_lf = content.contains('\n') && !content_crlf;
    let old_lf = old.contains('\n') && !old_crlf;

    if content_crlf && old_lf {
        return Some(
            "Note: line endings differ — file uses CRLF, `old_string` uses LF".to_string(),
        );
    }
    if old_crlf && content_lf {
        return Some(
            "Note: line endings differ — `old_string` uses CRLF, file uses LF".to_string(),
        );
    }

    // Fallback: line endings differ but neither side is clearly CRLF/LF-only.
    // 回退：换行风格不同，但无法明确归类为仅 CRLF 或仅 LF。
    Some("Note: line endings differ — try matching the file's line endings".to_string())
}

/// Finds the most similar line or multi-line window in `content` to `old`.
/// 在 `content` 中查找与 `old` 最相似的单行或多行连续片段。
///
/// # Description
/// Uses `similar::get_close_matches` on line windows. Windows include every
/// contiguous run of the same line count as `old`, so multi-line
/// `old_string`s are matched without quadratic cost. The returned candidate
/// is at least 90% similar, or `None` if nothing qualifies.
/// # 描述
/// 基于行窗口调用 `similar::get_close_matches`。窗口覆盖所有与 `old` 行数
/// 相同的连续片段，因此无需二次方开销即可匹配多行 `old_string`。仅当候选
/// 项相似度不低于 90% 时返回，否则返回 `None`。
fn closest_match(content: &str, old: &str) -> Option<String> {
    // Normalize line endings so CRLF vs LF differences do not distort the
    // character-level similarity ratio; the line-ending mismatch path has
    // already handled pure line-ending differences.
    // 规范化换行符，避免 CRLF/LF 差异干扰字符级相似度；纯换行差异已由
    // 上面的专用分支处理。
    let content = content.replace("\r\n", "\n");

    let lines: Vec<&str> = content.lines().collect();
    let want = old.lines().count().max(1);
    let mut candidates: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
    for window in lines.windows(want) {
        candidates.push(window.join("\n"));
    }

    let refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
    similar::get_close_matches(old, &refs, 1, 0.9)
        .first()
        .map(|matched| (*matched).to_string())
}

pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let plan = match plan_edit(params).await {
        Ok(plan) => plan,
        Err(message) => return ToolResult::failure("edit", message),
    };

    let new_content = if plan.replace_all {
        plan.content.replace(&plan.old_string, &plan.new_string)
    } else {
        plan.content.replacen(&plan.old_string, &plan.new_string, 1)
    };

    if let Err(e) = write_file(&plan.file_path, &new_content).await {
        return ToolResult::failure("edit", e.to_string());
    }

    let replaced = if plan.replace_all { plan.count } else { 1 };

    // 统计行数变化
    let old_lines = plan.content.lines().count();
    let new_lines = new_content.lines().count();
    let line_diff = new_lines as i64 - old_lines as i64;
    let line_info = if line_diff >= 0 {
        format!("+{line_diff} lines")
    } else {
        format!("{line_diff} lines")
    };

    // replace_all 时生成文件级 diff 以反映所有替换位置；单次替换直接用参数级 diff
    let diff = if plan.replace_all {
        generate_diff(&plan.content, &new_content)
    } else {
        generate_diff(&plan.old_string, &plan.new_string)
    };

    let message = format!(
        "Replaced {replaced} occurrence(s) in `{}` ({old_lines} -> {new_lines} lines, {line_info})\n```diff\n{diff}\n```",
        plan.file_path
    );
    ToolResult::success("edit", message, false)
}

/// 使用 `similar` crate 生成带上下文的 unified diff。
/// 最多展示 200 行 diff 以避免 token 浪费。
fn generate_diff(old: &str, new: &str) -> String {
    use similar::TextDiff;

    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();

    for op in diff.ops() {
        for change in diff.iter_changes(op) {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => '-',
                similar::ChangeTag::Insert => '+',
                similar::ChangeTag::Equal => ' ',
            };
            output.push(sign);
            output.push(' ');
            let value = change.value();
            output.push_str(value);
            if !value.ends_with('\n') {
                output.push('\n');
            }
        }
    }

    let lines: Vec<&str> = output.lines().collect();
    if lines.len() > 200 {
        format!(
            "{}\n... ({} more lines truncated)",
            lines[..200].join("\n"),
            lines.len() - 200
        )
    } else {
        output.trim_end().to_string()
    }
}
