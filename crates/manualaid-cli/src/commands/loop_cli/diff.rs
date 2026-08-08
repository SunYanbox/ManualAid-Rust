//! Colored unified-diff rendering shared by the Edit/Write approval
//! previews: build the diff once and color it line by line, so both tools
//! show identical full-length previews.
//! Edit/Write 审批预览共用的着色 unified diff 渲染：先生成 diff，再逐行着色，
//! 使两个工具展示一致的完整预览。

use crate::style;

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
                style::cyan(line)
            } else if line.starts_with('-') {
                style::red(line)
            } else if line.starts_with('+') {
                style::green(line)
            } else if line.starts_with(' ') {
                style::gray(line)
            } else {
                line.to_string()
            };
        out.push_str(&styled);
        out.push('\n');
    }
    out
}

/// Build a full colored unified diff between `original` and `modified`.
/// 构建 `original` 与 `modified` 之间的完整着色 unified diff。
pub(super) fn colored_diff(path: &str, original: &str, modified: &str) -> String {
    colorize_diff(&unified_diff(path, original, modified))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_uses_a_and_b_headers() {
        let diff = unified_diff("doc.txt", "a\nb\n", "a\nc\n");
        assert!(diff.contains("a/doc.txt"));
        assert!(diff.contains("b/doc.txt"));
        assert!(diff.contains("-b"));
        assert!(diff.contains("+c"));
    }

    #[test]
    fn colorize_diff_styles_each_line_kind() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        style::set_enabled(true);
        let colored = colorize_diff("@@ -1 +1 @@\n--- a/x\n+++ b/x\n-old\n+new\n context\nplain");
        assert!(colored.contains("\x1b["));
        assert!(
            !colored
                .lines()
                .any(|line| line.ends_with("plain") && line.contains("\x1b["))
        );
        style::set_enabled(false);
    }

    #[test]
    fn colored_diff_keeps_plain_text_without_style() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        style::set_enabled(false);
        let plain = colored_diff("doc.txt", "a\nb\n", "a\nc\n");
        assert!(!plain.contains("\x1b["));
        assert!(plain.contains("a/doc.txt"));
        assert!(plain.contains("-b"));
        assert!(plain.contains("+c"));
    }

    #[test]
    fn colored_diff_is_empty_when_texts_are_identical() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        style::set_enabled(false);
        assert!(colored_diff("doc.txt", "same\n", "same\n").is_empty());
    }

    #[test]
    fn colored_diff_handles_empty_inputs() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        style::set_enabled(false);
        assert!(colored_diff("doc.txt", "", "").is_empty());
        assert!(!colored_diff("doc.txt", "", "x\n").is_empty());
    }
}
