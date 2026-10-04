use super::*;

#[test]
fn line_count_handles_empty_and_newlines() {
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("a"), 1);
    assert_eq!(line_count("a\n"), 1);
    assert_eq!(line_count("a\nb\n"), 2);
    assert_eq!(line_count("a\r\nb\r\n"), 2);
}

#[test]
fn footer_without_offset_or_limit_reports_whole_file() {
    assert_eq!(read_footer("a\nb\n", 0, 0), "(whole file: 2 lines)");
    assert_eq!(read_footer("a\n", 0, 0), "(whole file: 1 line)");
    assert_eq!(read_footer("", 0, 0), "(whole file: 0 lines)");
}

#[test]
fn footer_with_offset_and_limit_suggests_next_offset() {
    assert_eq!(
        read_footer("a\nb\nc\n", 2, 1),
        "(lines 2-2 of 3; next offset: 3)"
    );
}

#[test]
fn footer_offset_read_to_end_has_no_next_offset() {
    assert_eq!(read_footer("a\nb\nc\n", 2, 0), "(lines 2-3 of 3)");
}

#[test]
fn footer_limit_only_starts_at_first_line() {
    assert_eq!(read_footer("a\nb\nc\n", 0, 1), "(lines 1-1 of 3)");
}

#[test]
fn footer_limit_past_end_caps_at_total() {
    assert_eq!(read_footer("a\nb\nc\n", 3, 5), "(lines 3-3 of 3)");
}

#[test]
fn footer_crlf_and_missing_trailing_newline() {
    assert_eq!(read_footer("a\r\nb\r\n", 0, 0), "(whole file: 2 lines)");
    assert_eq!(
        read_footer("a\nb", 1, 1),
        "(lines 1-1 of 2; next offset: 2)"
    );
}

#[test]
fn line_ending_summary_detects_lf_only() {
    assert_eq!(line_ending_summary("a\nb\n"), "EOL: LF");
    assert_eq!(line_ending_summary("a"), "EOL: LF");
    assert_eq!(line_ending_summary(""), "EOL: LF");
}

#[test]
fn line_ending_summary_detects_crlf_only() {
    assert_eq!(line_ending_summary("a\r\nb\r\n"), "EOL: CRLF");
}

#[test]
fn line_ending_summary_detects_mixed_endings() {
    assert_eq!(
        line_ending_summary("a\r\nb\n"),
        "EOL: mixed — set show_line_endings for a per-line check"
    );
}
