//! Write tool execution: writes content to a file, creating parent
//! directories as needed.
//! Write 工具执行：将内容写入文件，父目录缺失时自动创建。

use indexmap::IndexMap;
use serde_json::Value;

use super::{ToolResult, get_string};
use crate::async_fs::write_file;

/// Write one file parameter set.
/// 写入一个文件参数集。
pub(crate) async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let file_path = match get_string(params, "file_path") {
        Some(path) => path,
        None => return ToolResult::failure("write", "Missing required parameter `file_path`"),
    };
    let content = match get_string(params, "content") {
        Some(content) => content,
        None => return ToolResult::failure("write", "Missing required parameter `content`"),
    };

    match write_file(&file_path, &content).await {
        Ok(()) => ToolResult::success(
            "write",
            format!(
                "Written {} bytes ({} chars) to `{file_path}`",
                content.len(),
                content.chars().count()
            ),
            false,
        ),
        Err(e) => ToolResult::failure("write", e.to_string()),
    }
}
