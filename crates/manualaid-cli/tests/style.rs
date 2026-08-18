//! Integration tests for the public ANSI styling API.
//! ANSI 样式公共 API 的集成测试。

use std::sync::{Mutex, MutexGuard};

use manualaid_cli::style;

/// Serializes tests that depend on the process-wide styling switch.
/// 串行化依赖进程级样式开关的测试。
static STYLE_LOCK: Mutex<()> = Mutex::new(());

fn style_guard() -> MutexGuard<'static, ()> {
    STYLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn styling_is_disabled_by_default() {
    let _guard = style_guard();
    assert!(!style::is_enabled());
}

#[test]
fn set_enabled_toggles_the_switch() {
    let _guard = style_guard();
    style::set_enabled(true);
    assert!(style::is_enabled());
    style::set_enabled(false);
    assert!(!style::is_enabled());
}

#[test]
fn auto_init_disables_when_stdout_is_piped() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_manualaid-cli"))
        .output()
        .expect("run binary");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(text.contains(&format!(
        "ManualAid v{} is running",
        env!("CARGO_PKG_VERSION")
    )));
    assert!(!text.contains('\x1b'), "piped stdout must not be styled");
}

#[test]
fn text_styles_are_noops_when_disabled() {
    let _guard = style_guard();
    style::set_enabled(false);
    assert_eq!(style::bold("x"), "x");
    assert_eq!(style::dim("x"), "x");
    assert_eq!(style::italic("x"), "x");
    assert_eq!(style::underline("x"), "x");
    assert_eq!(style::strike("x"), "x");
    assert_eq!(style::red("x"), "x");
    assert_eq!(style::green("x"), "x");
    assert_eq!(style::yellow("x"), "x");
    assert_eq!(style::blue("x"), "x");
    assert_eq!(style::magenta("x"), "x");
    assert_eq!(style::cyan("x"), "x");
    assert_eq!(style::gray("x"), "x");
    assert_eq!(style::header("x"), "x");
    assert_eq!(style::muted("x"), "x");
    assert_eq!(style::success("x"), "x");
    assert_eq!(style::error("x"), "x");
    assert_eq!(style::accent("x"), "x");
}

#[test]
fn text_styles_wrap_with_expected_codes_when_enabled() {
    let _guard = style_guard();
    style::set_enabled(true);
    assert_eq!(style::bold("x"), "\x1b[1mx\x1b[0m");
    assert_eq!(style::dim("x"), "\x1b[2mx\x1b[0m");
    assert_eq!(style::italic("x"), "\x1b[3mx\x1b[0m");
    assert_eq!(style::underline("x"), "\x1b[4mx\x1b[0m");
    assert_eq!(style::strike("x"), "\x1b[9mx\x1b[0m");
    assert_eq!(style::red("x"), "\x1b[31mx\x1b[0m");
    assert_eq!(style::green("x"), "\x1b[32mx\x1b[0m");
    assert_eq!(style::yellow("x"), "\x1b[33mx\x1b[0m");
    assert_eq!(style::blue("x"), "\x1b[34mx\x1b[0m");
    assert_eq!(style::magenta("x"), "\x1b[35mx\x1b[0m");
    assert_eq!(style::cyan("x"), "\x1b[36mx\x1b[0m");
    assert_eq!(style::gray("x"), "\x1b[90mx\x1b[0m");
    assert_eq!(style::header("x"), "\x1b[1;36mx\x1b[0m");
    assert_eq!(style::muted("x"), "\x1b[90mx\x1b[0m");
    assert_eq!(style::success("x"), "\x1b[32mx\x1b[0m");
    assert_eq!(style::error("x"), "\x1b[1;31mx\x1b[0m");
    assert_eq!(style::accent("x"), "\x1b[36mx\x1b[0m");
    style::set_enabled(false);
}

#[test]
fn strip_ansi_removes_csi_sequences() {
    let _guard = style_guard();
    style::set_enabled(true);
    let styled = format!(
        "{}\n{}\n{}",
        style::header("Masked text"),
        style::error("boom"),
        style::dim("total: 3"),
    );
    assert_eq!(style::strip_ansi(&styled), "Masked text\nboom\ntotal: 3");
    style::set_enabled(false);
}

#[test]
fn strip_ansi_handles_escape_edges() {
    let _guard = style_guard();
    assert_eq!(style::strip_ansi("\x1b[1;36ma\x1b[0m b"), "a b");
    assert_eq!(style::strip_ansi("\x1b[31m"), "");
    assert_eq!(style::strip_ansi("\x1b["), "");
    assert_eq!(style::strip_ansi("\x1bX"), "X");
    assert_eq!(style::strip_ansi("\x1b"), "");
    assert_eq!(style::strip_ansi("plain"), "plain");
    assert_eq!(style::strip_ansi(""), "");
}
