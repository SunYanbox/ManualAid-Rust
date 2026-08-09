//! Context file selection for system-prompt generation.
//! 生成系统提示词时上下文文件的选择。

use std::path::{Path, PathBuf};

use manualaid_ws::context::{ContextFile, discover_context_files, duplicate_of};

use super::utils::{read_line, t_fmt};

/// Discover context files at `root` and return the ones to load: nothing
/// when none exist, the single file when exactly one exists, and the user's
/// menu selection when several exist. An empty or invalid selection loads
/// nothing. Called when the system prompt is generated so the question is
/// only asked when it is relevant.
/// 发现 `root` 下的上下文文件并返回要加载的文件：无文件时返回空，仅一个文件时直接加载，
/// 多个文件时返回用户菜单选择结果。空输入或无效输入不加载任何文件。在生成系统提示词时
/// 调用，因此只在需要时提问。
pub(super) fn select_context_files(root: &Path) -> Vec<PathBuf> {
    let files = discover_context_files(root);
    match files.len() {
        0 => Vec::new(),
        1 => vec![files[0].path.clone()],
        _ => menu_select(&files),
    }
}

/// Render the multi-file selection menu, marking files whose content is
/// identical to an earlier entry, and read one line of indices.
/// 渲染多文件选择菜单（标记与更早条目内容相同的文件）并读取一行索引输入。
fn menu_select(files: &[ContextFile]) -> Vec<PathBuf> {
    crate::console::out_println!("{}", i18n::t_str("cli.context.found_multiple"));
    let duplicates = duplicate_of(files);
    for (index, file) in files.iter().enumerate() {
        let duplicate_note = duplicates[index]
            .map(|duplicate_index| {
                let name = file_name(&files[duplicate_index].path);
                t_fmt("cli.context.duplicate_note", &[("name", name)])
            })
            .unwrap_or_default();
        crate::console::out_println!(
            "{}",
            t_fmt(
                "cli.context.item",
                &[
                    ("index", &(index + 1).to_string()),
                    ("name", file_name(&file.path)),
                    ("size", &format_size(file.size)),
                    ("duplicate_note", &duplicate_note),
                ],
            )
        );
    }
    crate::console::out_print!("{}", i18n::t_str("cli.context.prompt"));
    crate::console::flush();
    let line = read_line().unwrap_or_default();
    let indices = parse_selection(&line, files.len());
    if indices.is_empty() {
        return Vec::new();
    }
    let paths: Vec<PathBuf> = indices
        .into_iter()
        .map(|index| files[index].path.clone())
        .collect();
    let names = paths
        .iter()
        .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
        .collect::<Vec<_>>()
        .join(", ");
    crate::console::out_println!("{}", t_fmt("cli.context.loaded", &[("names", &names)]));
    paths
}

/// Parse a user-selected index list like `1,2` or `1 2`. Out-of-range
/// tokens are ignored; an empty or non-numeric input selects nothing.
/// 解析用户输入的索引列表，例如 `1,2` 或 `1 2`。越界项被忽略，空输入或非数字输入不选择任何文件。
fn parse_selection(input: &str, count: usize) -> Vec<usize> {
    input
        .trim()
        .split([',', ' ', '\t'])
        .filter_map(|token| token.parse::<usize>().ok())
        .filter_map(|index| index.checked_sub(1))
        .filter(|index| *index < count)
        .collect()
}

/// The file name of `path`, or an empty string when it has none.
/// `path` 的文件名；没有文件名时返回空字符串。
fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

/// Format a byte size for the menu, e.g. `3.9 KB`.
/// 格式化菜单中的文件大小，例如 `3.9 KB`。
fn format_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kib = bytes as f64 / KIB;
    if kib < KIB {
        return format!("{kib:.1} KB");
    }
    format!("{:.1} MB", kib / KIB)
}

#[cfg(test)]
mod tests {
    use super::super::utils::push_test_input;
    use super::*;

    #[test]
    fn parse_selection_handles_commas_spaces_and_range() {
        assert_eq!(parse_selection("1,2", 3), vec![0, 1]);
        assert_eq!(parse_selection("1 3", 3), vec![0, 2]);
        assert_eq!(parse_selection("1\t2", 3), vec![0, 1]);
        assert_eq!(parse_selection("", 3), Vec::<usize>::new());
        assert_eq!(parse_selection("1\r\n", 3), vec![0]);
        assert_eq!(parse_selection("abc", 3), Vec::<usize>::new());
        assert_eq!(parse_selection("0", 3), Vec::<usize>::new());
        assert_eq!(parse_selection("4", 3), Vec::<usize>::new());
        assert_eq!(parse_selection("2,99", 3), vec![1]);
    }

    #[test]
    fn format_size_renders_binary_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(3994), "3.9 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn select_context_files_skips_when_none_exist() {
        let root = crate::test_support::temp_dir("ctx-none");
        assert!(select_context_files(&root).is_empty());
    }

    #[test]
    fn select_context_files_auto_loads_the_single_file() {
        let root = crate::test_support::temp_dir("ctx-single");
        std::fs::write(root.join("AGENTS.md"), "rules").unwrap();
        let selected = select_context_files(&root);
        assert_eq!(selected, vec![root.join("AGENTS.md")]);
    }

    #[test]
    fn menu_select_loads_chosen_indices_and_marks_duplicates() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let root = crate::test_support::temp_dir("ctx-menu");
        std::fs::write(root.join("AGENTS.md"), "same").unwrap();
        std::fs::write(root.join("CLAUDE.md"), "same").unwrap();
        std::fs::write(root.join("GEMINI.md"), "other").unwrap();
        let files = discover_context_files(&root);
        let duplicates = duplicate_of(&files);
        assert_eq!(duplicates, vec![None, None, Some(0)]);
        push_test_input(&["1,3"]);
        let selected = menu_select(&files);
        assert_eq!(selected, vec![files[0].path.clone(), files[2].path.clone()]);
    }

    #[test]
    fn menu_select_empty_input_loads_nothing() {
        let _capture = crate::console::capture();
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        let root = crate::test_support::temp_dir("ctx-menu-empty");
        std::fs::write(root.join("AGENTS.md"), "a").unwrap();
        std::fs::write(root.join("CLAUDE.md"), "b").unwrap();
        let files = discover_context_files(&root);
        push_test_input(&[""]);
        assert!(menu_select(&files).is_empty());
    }
}
