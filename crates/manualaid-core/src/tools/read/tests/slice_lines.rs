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
