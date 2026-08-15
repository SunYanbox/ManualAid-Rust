use manualaid_cli::commands::debug::shell::run_shell_debug_with_confirm;
use std::path::PathBuf;
use std::fs;

fn temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{}-{}", prefix, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn aborts_before_executing_when_confirmation_is_no() {
    let dir = temp_dir("shell-abort");
    let marker = dir.join("marker.txt");
    let command = if cfg!(windows) {
        format!("type nul > {}", marker.display())
    } else {
        format!("touch {}", marker.display())
    };
    let err = run_shell_debug_with_confirm(&command, None, || Some("n".to_string()))
        .await
        .unwrap_err();
    assert!(err.contains("Aborted."));
    assert!(!marker.exists(), "command must not run after abort");
}

#[tokio::test]
async fn denied_blacklist_command_never_reaches_confirm_or_execution() {
    let err = run_shell_debug_with_confirm("rm -rf /", None, || {
        panic!("confirmation must not be requested for a denied command")
    })
    .await
    .unwrap_err();
    assert!(err.contains("denied"), "unexpected error: {err}");
}

#[tokio::test]
async fn rejects_empty_command_before_audit() {
    let err = run_shell_debug_with_confirm("   ", None, || {
        panic!("confirmation must not be requested for an empty command")
    })
    .await
    .unwrap_err();
    assert!(err.contains("must not be empty"));
}

#[tokio::test]
async fn rejects_invalid_timeout() {
    let err = run_shell_debug_with_confirm("echo hi", Some("abc"), || {
        panic!("confirmation must not be requested for an invalid timeout")
    })
    .await
    .unwrap_err();
    assert!(err.contains("Invalid timeout"));
}