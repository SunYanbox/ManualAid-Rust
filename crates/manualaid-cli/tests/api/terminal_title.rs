//! API tests: terminal window title derivation.
//! API 测试：终端窗口标题的推导。

use std::path::Path;

use manualaid_cli::terminal_title::project_title;

#[test]
fn project_title_uses_the_last_path_segment() {
    assert_eq!(project_title(Path::new("a/b/demo")), "[ManualAid] demo");
    assert_eq!(project_title(Path::new("demo")), "[ManualAid] demo");
    assert_eq!(project_title(Path::new("a/b/demo/")), "[ManualAid] demo");
}

#[cfg(unix)]
#[test]
fn project_title_uses_the_last_segment_of_absolute_paths() {
    assert_eq!(
        project_title(Path::new("/home/me/demo")),
        "[ManualAid] demo"
    );
}

#[cfg(windows)]
#[test]
fn project_title_uses_the_last_segment_of_absolute_paths() {
    assert_eq!(
        project_title(Path::new(r"C:\Users\me\demo")),
        "[ManualAid] demo"
    );
}

#[test]
fn project_title_falls_back_without_a_usable_segment() {
    assert_eq!(project_title(Path::new("")), "[ManualAid]");
    assert_eq!(project_title(Path::new("..")), "[ManualAid]");
}

#[cfg(unix)]
#[test]
fn project_title_falls_back_for_the_filesystem_root() {
    assert_eq!(project_title(Path::new("/")), "[ManualAid]");
}

#[cfg(windows)]
#[test]
fn project_title_falls_back_for_drive_roots() {
    assert_eq!(project_title(Path::new(r"C:\")), "[ManualAid]");
    assert_eq!(project_title(Path::new("C:")), "[ManualAid]");
}

#[test]
fn project_title_strips_control_characters() {
    // A crafted folder name must not end the OSC sequence early (BEL) or
    // start another escape sequence (ESC).
    // 恶意文件夹名不得提前结束 OSC 序列（BEL）或开启新的转义序列（ESC）。
    assert_eq!(
        project_title(Path::new("bad\x1b]2;evil\x07name")),
        "[ManualAid] bad]2;evilname"
    );
}

#[test]
fn project_title_falls_back_when_sanitizing_leaves_nothing() {
    assert_eq!(project_title(Path::new("\x07")), "[ManualAid]");
}

#[test]
fn project_title_keeps_unicode_folder_names() {
    assert_eq!(project_title(Path::new("项目-α")), "[ManualAid] 项目-α");
}
