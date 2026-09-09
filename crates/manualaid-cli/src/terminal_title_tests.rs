//! Unit tests for the private title-writing paths: ANSI rendering and the
//! capture/terminal guards that keep `cargo test` from touching the real
//! window title.
//! 私有标题写入路径的单元测试：ANSI 渲染，以及使 `cargo test` 不会改动真实
//! 窗口标题的捕获/终端守卫。

use super::*;

#[test]
fn render_title_matches_the_crossterm_ansi_sequence() {
    assert_eq!(
        render_title("[ManualAid] demo"),
        "\x1B]0;[ManualAid] demo\x07"
    );
}

#[test]
fn set_project_title_emits_the_sequence_into_the_capture() {
    let capture = crate::console::capture();
    set_project_title(Path::new("demo"));
    assert_eq!(capture.text(), "\x1B]0;[ManualAid] demo\x07");
}

#[test]
fn write_title_to_terminal_is_a_noop_in_test_builds() {
    // Called directly so the test never races a concurrently held capture:
    // `write_title` would route into that buffer and pollute its assertion.
    // 直接调用该函数，避免与并发持有的捕获竞争：经 `write_title` 会写入
    // 对方的缓冲区并破坏其断言。
    write_title_to_terminal("[ManualAid] demo");
}
