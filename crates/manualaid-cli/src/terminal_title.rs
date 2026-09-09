//! Terminal window title: identifies the running session by its project
//! folder so parallel ManualAid sessions on different projects can be told
//! apart in the window/tab title bar.
//! 终端窗口标题：以项目文件夹标识当前会话，便于在窗口/标签标题栏中区分
//! 不同项目的并行 ManualAid 会话。

use std::io::{self, IsTerminal};
use std::path::Path;

use crossterm::Command;
use crossterm::terminal::SetTitle;

/// Prefix identifying a ManualAid window in the title bar, including the
/// separating space before the folder name.
/// 标题栏中标识 ManualAid 窗口的前缀，含与文件夹名之间的分隔空格。
const TITLE_PREFIX: &str = "[ManualAid] ";

/// Title used when the project root has no usable folder name.
/// 项目根没有可用文件夹名时使用的标题。
const FALLBACK_TITLE: &str = "[ManualAid]";

/// The window title for `project_root`: `[ManualAid] <folder>`, or
/// [`FALLBACK_TITLE`] when the last path segment is missing or unusable.
/// 项目 `project_root` 的窗口标题：`[ManualAid] <文件夹名>`；最后一段缺失或
/// 不可用时返回 [`FALLBACK_TITLE`]。
///
/// # Description
/// Control characters are stripped because the title is embedded in an ANSI
/// OSC sequence: a BEL would end it early and an ESC could start another
/// escape sequence, letting a crafted folder name rewrite terminal state.
/// # 描述
/// 控制字符会被剔除：标题会嵌入 ANSI OSC 序列，BEL 会提前结束该序列，
/// ESC 则可能开启新的转义序列，恶意文件夹名因此可以改写终端状态。
pub fn project_title(project_root: &Path) -> String {
    let folder = project_root
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let sanitized: String = folder.chars().filter(|c| !c.is_control()).collect();
    if sanitized.is_empty() {
        return FALLBACK_TITLE.to_string();
    }
    format!("{TITLE_PREFIX}{sanitized}")
}

/// Set the terminal window title to the project's [`project_title`].
/// 把终端窗口标题设为该项目的 [`project_title`]。
pub(crate) fn set_project_title(project_root: &Path) {
    write_title(&project_title(project_root));
}

/// Write `title` to the terminal, honoring the process-wide console capture
/// so tests never touch the real window title.
/// 把 `title` 写入终端；尊重进程级控制台捕获，测试因此不会改动真实窗口标题。
fn write_title(title: &str) {
    // Capture first: unit tests observe the emitted bytes only through the
    // capture buffer, and the test-build guard below would otherwise skip
    // the write before it could be asserted.
    // 先判断捕获：单元测试只能通过捕获缓冲区观察输出字节；若先判断下方的
    // 测试构建守卫，写入会在断言前就被跳过。
    if crate::console::is_capturing() {
        crate::console::write_text(&render_title(title));
        return;
    }
    write_title_to_terminal(title);
}

/// Write `title` to the attached terminal; a no-op in test builds and when
/// stdout is redirected, so tests and pipelines never receive the sequence.
/// 有终端时把 `title` 写入终端；测试构建与 stdout 被重定向时不写，测试与
/// 管道因此不会收到该序列。
fn write_title_to_terminal(title: &str) {
    // Unit tests can see the real terminal, so rewriting the user's window
    // title during `cargo test` would be a visible side effect.
    // 单元测试能看见真实终端，测试期间改写用户窗口标题会是可见副作用。
    if cfg!(test) || !io::stdout().is_terminal() {
        return;
    }
    // Keep the command API here instead of the rendered string: crossterm
    // picks ANSI or the Windows WinAPI fallback, so legacy consoles work too.
    // 此处保留命令 API 而非渲染后的字符串：crossterm 会选择 ANSI 或 Windows
    // WinAPI 回退，旧版控制台同样可用。
    let _ = crossterm::execute!(io::stdout(), SetTitle(title));
}

/// Render the `SetTitle` ANSI sequence for `title`.
/// 把 `title` 的 `SetTitle` ANSI 序列渲染为字符串。
fn render_title(title: &str) -> String {
    let mut sequence = String::new();
    let _ = SetTitle(title).write_ansi(&mut sequence);
    sequence
}

#[cfg(test)]
#[path = "terminal_title_tests.rs"]
mod tests;
