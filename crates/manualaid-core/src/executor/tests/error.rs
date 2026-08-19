use super::*;

#[test]
fn executor_error_display_includes_tool() {
    let error = ExecutorError::new("boom", "read");
    assert_eq!(error.to_string(), "[read] boom");
}
