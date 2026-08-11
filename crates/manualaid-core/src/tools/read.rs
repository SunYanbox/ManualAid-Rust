//! Read tool execution: reads a file with encoding detection and optional
//! `offset` / `limit` line slicing. The pre-check shares its readability
//! validation with the execution path so calls that are guaranteed to fail
//! never reach the approval queue.
//! Read 工具执行：读取文件（带编码检测），支持 `offset` / `limit` 行切片。

use indexmap::IndexMap;
use serde_json::Value;

use super::{ToolResult, get_i64, get_string};
use crate::async_fs::read_file;

/// Read one file parameter set.
/// 读取一个文件参数集。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let file_path = match get_string(params, "file_path") {
        Some(path) => path,
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

    let output = match slice_lines(&content, offset, limit) {
        Ok(output) => output,
        Err(message) => return ToolResult::failure("read", message),
    };
    ToolResult::success("read", output, true)
}

/// Pre-check one read call for guaranteed failure before it reaches the
/// approval queue. A directory is rejected with an explicit message; other
/// unreadable paths report the underlying I/O error.
pub(crate) async fn pre_check(params: &IndexMap<String, Value>) -> Result<(), String> {
    let file_path = get_string(params, "file_path")
        .ok_or_else(|| "Missing required parameter `file_path`".to_string())?;
    if std::path::Path::new(&file_path).is_dir() {
        return Err("`file_path` is a directory; cannot read it as a file".to_string());
    }
    read_file(&file_path)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_slice_returns_whole_content() {
        assert_eq!(slice_lines("a\nb\n", 0, 0).unwrap(), "a\nb\n");
    }

    #[test]
    fn offset_slices_from_one_based_line() {
        assert_eq!(slice_lines("a\nb\nc\n", 2, 0).unwrap(), "b\nc\n");
    }

    #[test]
    fn limit_caps_the_slice() {
        assert_eq!(slice_lines("a\nb\nc\n", 1, 2).unwrap(), "a\nb\n");
    }

    #[test]
    fn offset_beyond_total_is_an_error() {
        assert!(slice_lines("a\n", 5, 0).is_err());
    }

    #[test]
    fn offset_zero_with_limit_starts_at_beginning() {
        assert_eq!(slice_lines("a\nb\n", 0, 1).unwrap(), "a\n");
    }
}
