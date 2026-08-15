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
        return Err(format!(
            "`old_string` not found in `{file_path}` — it may have already been applied \
             or the content has changed. Use the `read` tool to re-read `{file_path}` \
             and re-issue the edit with the current content.\n\
             String:\n{old_string}"
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
