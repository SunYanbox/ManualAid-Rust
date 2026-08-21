//! Read tool execution: reads a file with encoding detection and optional
//! `offset` / `limit` line slicing. The pre-check shares its readability
//! validation with the execution path so calls that are guaranteed to fail
//! never reach the approval queue.
//! Read 工具执行：读取文件（带编码检测），支持 `offset` / `limit` 行切片。

use indexmap::IndexMap;
use serde_json::Value;

use super::{ToolResult, get_bool, get_i64, get_string};
use crate::async_fs::read_file;

/// Read one file parameter set.
/// 读取一个文件参数集。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let file_path = match get_string(params, "file_path") {
        Some(path) => path.trim().to_string(),
        None => return ToolResult::failure("read", "Missing required parameter `file_path`"),
    };

    let content = match read_file(&file_path).await {
        Ok(content) => content,
        Err(e) => return ToolResult::failure("read", e.to_string()),
    };

    let offset = get_i64(params, "offset").unwrap_or(0);
    let limit = get_i64(params, "limit").unwrap_or(0);

    if offset < 0 {
        return ToolResult::failure("read", "`offset` must be >= 0");
    }
    if limit < 0 {
        return ToolResult::failure("read", "`limit` must be >= 0");
    }

    let sliced = match slice_lines(&content, offset, limit) {
        Ok(output) => output,
        Err(message) => return ToolResult::failure("read", message),
    };

    let show_line_numbers = get_bool(params, "show_line_numbers").unwrap_or(false);
    let show_line_endings = get_bool(params, "show_line_endings").unwrap_or(false);

    let mut output = if show_line_numbers || show_line_endings {
        decorate(&sliced, offset, show_line_numbers, show_line_endings)
    } else {
        sliced
    };

    // Append the range/count marker as a separate line so the model can
    // tell how much of the file was returned and how to continue.
    // 将范围/行数标记作为独立行追加，使模型知道返回了文件的多少内容以及如何继续。
    let footer = read_footer(&content, offset, limit);
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push_str(&footer);
    output.push('\n');

    ToolResult::success("read", output, true)
}

/// Pre-check one read call for guaranteed failure before it reaches the
/// approval queue. A directory is rejected with an explicit message; other
/// unreadable paths report the underlying I/O error.
pub(crate) async fn pre_check(params: &IndexMap<String, Value>) -> Result<(), String> {
    let file_path = get_string(params, "file_path")
        .ok_or_else(|| "Missing required parameter `file_path`".to_string())?
        .trim()
        .to_string();
    if std::path::Path::new(&file_path).is_dir() {
        return Err("`file_path` is a directory; cannot read it as a file".to_string());
    }
    read_file(&file_path)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Count non-empty lines using `split('\n')`: a trailing newline yields one
/// extra empty segment, so it is removed before counting.
/// 使用 `split('\n')` 统计非空行数：末尾换行会多出一个空段，因此先移除再计数。
fn line_count(content: &str) -> usize {
    let trimmed = content.trim_end_matches('\n');
    if trimmed.is_empty() {
        return 0;
    }
    trimmed.split('\n').count()
}

/// Build the trailing range/count marker appended to a read result.
/// 构造追加在读取结果末尾的范围/行数标记。
fn read_footer(content: &str, offset: i64, limit: i64) -> String {
    let total = line_count(content) as i64;

    if offset == 0 && limit == 0 {
        return format!("(End of file - total {total} lines)");
    }

    let start = if offset > 0 { offset } else { 1 };
    let end = if limit > 0 {
        (start + limit - 1).min(total)
    } else {
        total
    };
    let has_more = offset > 0 && end < total;

    if has_more {
        format!(
            "(Showing lines {start}-{end} of {total} lines. Use offset={} to continue.)",
            end + 1
        )
    } else {
        // Includes `limit` extending past the end, `offset > 0, limit == 0`
        // reading to the end, and `limit` without an offset, none of which
        // have a meaningful next offset to continue to.
        // 包括 `limit` 超出末尾、`offset > 0, limit == 0` 读到末尾，以及
        // 只给 `limit` 的情况；这些场景都不存在可继续的下一个 offset。
        format!("(Showing lines {start}-{end} of {total} lines)")
    }
}

/// Slice `content` by 1-based `offset` and optional `limit`.
/// 按 1 起始的 `offset` 与可选的 `limit` 对 `content` 进行切片。
fn slice_lines(content: &str, offset: i64, limit: i64) -> Result<String, String> {
    if offset == 0 && limit == 0 {
        return Ok(content.to_string());
    }
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let total = lines.len();
    if offset > 0 && offset as usize > total {
        return Err(format!(
            "`offset` {offset} exceeds total line count {total}"
        ));
    }
    let start = (offset.max(1) - 1) as usize;
    let end = if limit > 0 {
        (start + limit as usize).min(total)
    } else {
        total
    };
    Ok(lines[start..end].join(""))
}

/// Decorate a sliced read output with right-aligned line numbers and
/// `cat -E`-style line-ending markers. The number prefix uses a fixed-width
/// right-aligned field followed by `| ` so the model can reliably judge line
/// lengths regardless of tabs. The default read path returns the raw slice;
/// this function is only invoked when at least one diagnostic switch is
/// enabled.
/// 用右对齐的行号与 `cat -E` 风格的行尾标记装饰已切片的读取输出。
/// 行号前缀使用定宽右对齐字段并后接 `| `，使模型无需依赖制表符即可可靠判断行宽。
/// 默认读取路径返回原始切片；仅当至少一个诊断开关启用时才调用本函数。
fn decorate(sliced: &str, offset: i64, show_line_numbers: bool, show_line_endings: bool) -> String {
    if sliced.is_empty() {
        return sliced.to_string();
    }

    let mut output = String::new();
    let start_line_no = if offset > 0 { offset } else { 1 };
    let last_line_no = start_line_no + sliced.split_inclusive('\n').count() as i64 - 1;
    let width = last_line_no.to_string().len();

    for (line_no, segment) in (start_line_no..).zip(sliced.split_inclusive('\n')) {
        let (content, ending) = match segment.strip_suffix('\n') {
            Some(body) => {
                if let Some(lf_body) = body.strip_suffix('\r') {
                    (lf_body, Some("\r\n"))
                } else {
                    (body, Some("\n"))
                }
            }
            None => (segment, None),
        };

        if show_line_numbers {
            output.push_str(&format!("{:>width$}| ", line_no, width = width));
        }
        output.push_str(content);

        if show_line_endings {
            if ending == Some("\r\n") {
                output.push_str("^M$");
            } else if ending == Some("\n") {
                output.push('$');
            }
        }
        if let Some(ending) = ending {
            output.push_str(ending);
        }
    }

    output
}

#[cfg(test)]
mod tests;
