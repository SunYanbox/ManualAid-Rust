//! Context file discovery, loading and rendering for the system prompt.
//! 系统提示词所需上下文文件的发现、加载与渲染。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Context file names checked at the workspace root, in load order.
/// 按加载顺序在工作区根目录检查的上下文文件名。
pub const CONTEXT_FILE_NAMES: &[&str] = &["AGENTS.md", "GEMINI.md", "CONVENTIONS.md", "CLAUDE.md"];

/// A context file found at the workspace root with its byte size, so the
/// CLI selection menu can show a human-readable size without re-reading it.
/// 工作区根目录发现的上下文文件及其字节大小，供 CLI 选择菜单直接展示可读大小。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextFile {
    /// Path of the file.
    /// 文件路径。
    pub path: PathBuf,
    /// Byte size of the file.
    /// 文件字节大小。
    pub size: u64,
}

/// Discover existing context files at `workspace_root` in the canonical
/// name order. Missing files and non-file entries are skipped silently.
/// 按规范名称顺序发现 `workspace_root` 下已存在的上下文文件；缺失文件与非普通文件条目静默跳过。
pub fn discover_context_files(workspace_root: &Path) -> Vec<ContextFile> {
    CONTEXT_FILE_NAMES
        .iter()
        .filter_map(|name| {
            let path = workspace_root.join(name);
            let metadata = std::fs::metadata(&path).ok()?;
            metadata.is_file().then_some(ContextFile {
                path,
                size: metadata.len(),
            })
        })
        .collect()
}

/// Read a context file as UTF-8 text. Unreadable or non-UTF-8 files are
/// treated as absent so one broken file never breaks prompt building.
/// 将上下文文件按 UTF-8 读取。不可读或非 UTF-8 文件按缺失处理，避免单个损坏文件破坏提示词构建。
fn read_context_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Render one `<context_files>` block per given file. Files that cannot be
/// read are skipped, and every block ends with a newline.
/// 为给定文件逐个渲染 `<context_files>` 区块。无法读取的文件被跳过，每个区块以换行结尾。
pub fn render_context_files(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for path in paths {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(content) = read_context_file(path) else {
            continue;
        };
        out.push_str(&format!("<context_files path=\"{name}\">\n"));
        out.push_str(&content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("</context_files>\n");
    }
    out
}

/// For each file, the index of the earliest file with identical content,
/// or `None` when no earlier file matches. Unreadable files never carry a
/// duplicate mark.
/// 返回每个文件对应的最早内容相同文件的索引；没有更早的相同文件时返回 `None`。不可读文件不标记重复。
pub fn duplicate_of(files: &[ContextFile]) -> Vec<Option<usize>> {
    let contents: Vec<Option<String>> = files
        .iter()
        .map(|file| read_context_file(&file.path))
        .collect();
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    let mut result = Vec::with_capacity(files.len());
    for (index, content) in contents.iter().enumerate() {
        let duplicate = content
            .as_ref()
            .and_then(|content| first_seen.get(content).copied());
        result.push(duplicate);
        if let Some(content) = content {
            first_seen.entry(content.clone()).or_insert(index);
        }
    }
    result
}
