use super::*;

#[test]
fn decorate_no_flags_returns_original() {
    assert_eq!(decorate("a\nb\n", 0, false, false), "a\nb\n");
}

#[test]
fn decorate_empty_slice_returns_empty() {
    assert_eq!(decorate("", 0, true, true), "");
}

#[test]
fn decorate_line_numbers_from_one_without_offset() {
    assert_eq!(decorate("a\nb\n", 0, true, false), "1| a\n2| b\n");
}

#[test]
fn decorate_line_numbers_start_at_offset() {
    assert_eq!(decorate("a\nb\n", 5, true, false), "5| a\n6| b\n");
}

#[test]
fn decorate_line_numbers_right_aligns() {
    assert_eq!(
        decorate("a\nb\nc\n", 9, true, false),
        " 9| a\n10| b\n11| c\n"
    );
}

#[test]
fn decorate_line_endings_marks_lf_with_dollar() {
    assert_eq!(decorate("a\nb\n", 0, false, true), "a$\nb$\n");
}

#[test]
fn decorate_line_endings_marks_crlf_with_caret_m_dollar() {
    assert_eq!(decorate("a\r\nb\r\n", 0, false, true), "a^M$\r\nb^M$\r\n");
}

#[test]
fn decorate_line_endings_skips_missing_trailing_newline() {
    assert_eq!(decorate("a\nb", 0, false, true), "a$\nb");
}

#[test]
fn decorate_combined_line_numbers_and_endings() {
    assert_eq!(decorate("a\r\nb\n", 0, true, true), "1| a^M$\r\n2| b$\n");
}
