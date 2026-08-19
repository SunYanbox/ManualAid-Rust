use super::*;

#[test]
fn format_output_combines_streams() {
    let result = crate::shell::CommandResult {
        stdout: "out".into(),
        stderr: "err".into(),
        exit_code: Some(0),
        signal: None,
        timed_out: false,
    };
    assert_eq!(format_output(&result), "out\nerr\n");
}

#[test]
fn format_output_flags_timeout() {
    let result = crate::shell::CommandResult {
        stdout: String::new(),
        stderr: "slow".into(),
        exit_code: None,
        signal: None,
        timed_out: true,
    };
    assert_eq!(format_output(&result), "slow\nCommand timed out\n");
}
