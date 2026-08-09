//! Integration tests for context file discovery, rendering and duplicate
//! detection.
//! 上下文文件发现、渲染与重复检测的集成测试。

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use manualaid_ws::context::{
    CONTEXT_FILE_NAMES, ContextFile, discover_context_files, duplicate_of, render_context_files,
};

/// A unique temporary root directory.
/// 唯一临时根目录。
fn temp_root(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-ws-context-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn discover_finds_existing_files_in_canonical_order() {
    let root = temp_root("order");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "a").unwrap();
    std::fs::write(root.join("CLAUDE.md"), "c").unwrap();
    std::fs::write(root.join("GEMINI.md"), "g").unwrap();
    let files = discover_context_files(&root);
    let names: Vec<&str> = files
        .iter()
        .map(|file| file.path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(names, vec!["AGENTS.md", "GEMINI.md", "CLAUDE.md"]);
    assert_eq!(files[0].size, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn discover_skips_missing_and_directory_entries() {
    let root = temp_root("skip");
    std::fs::create_dir_all(root.join("CLAUDE.md")).unwrap();
    let files = discover_context_files(&root);
    assert!(files.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn discover_returns_empty_for_empty_root() {
    let root = temp_root("empty");
    std::fs::create_dir_all(&root).unwrap();
    assert!(discover_context_files(&root).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn render_wraps_content_and_adds_trailing_newline() {
    let root = temp_root("render");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "# rules\n").unwrap();
    std::fs::write(root.join("CLAUDE.md"), "no trailing newline").unwrap();
    let text = render_context_files(&[root.join("AGENTS.md"), root.join("CLAUDE.md")]);
    assert!(text.contains("<context_files path=\"AGENTS.md\">\n# rules\n</context_files>"));
    assert!(
        text.contains("<context_files path=\"CLAUDE.md\">\nno trailing newline\n</context_files>")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn render_skips_missing_and_non_utf8_files() {
    let root = temp_root("render-skip");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("broken.md"), [0xFF, 0xFE, 0x00, 0x41]).unwrap();
    let text = render_context_files(&[root.join("missing.md"), root.join("broken.md")]);
    assert!(text.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn render_skips_paths_without_a_file_name() {
    assert_eq!(render_context_files(&[PathBuf::from("")]), "");
}

#[test]
fn render_empty_input_returns_empty_string() {
    assert_eq!(render_context_files(&[]), "");
}

#[test]
fn duplicate_of_marks_identical_content_with_earliest_index() {
    let root = temp_root("dup");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("AGENTS.md"), "same").unwrap();
    std::fs::write(root.join("GEMINI.md"), "same").unwrap();
    std::fs::write(root.join("CLAUDE.md"), "other").unwrap();
    let files = discover_context_files(&root);
    assert_eq!(duplicate_of(&files), vec![None, Some(0), None]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn duplicate_of_unreadable_file_has_no_mark() {
    let files = vec![ContextFile {
        path: PathBuf::from("missing-a.md"),
        size: 0,
    }];
    assert_eq!(duplicate_of(&files), vec![None]);
}

#[test]
fn context_file_names_are_stable_and_non_empty() {
    assert!(CONTEXT_FILE_NAMES.contains(&"AGENTS.md"));
    assert!(CONTEXT_FILE_NAMES.contains(&"CLAUDE.md"));
    assert!(CONTEXT_FILE_NAMES.contains(&"GEMINI.md"));
    assert!(CONTEXT_FILE_NAMES.contains(&"CONVENTIONS.md"));
}
