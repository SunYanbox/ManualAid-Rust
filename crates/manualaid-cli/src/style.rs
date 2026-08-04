//! # Description
//! ANSI escape styling for console output: colors, bold, dim and related
//! helpers, controlled by a process-wide switch that defaults to disabled
//! and is enabled automatically when stdout is a terminal and `NO_COLOR`
//! is unset.
//! # 描述
//! 控制台输出的 ANSI 转义样式：颜色、加粗、弱化等辅助函数，由进程级开关控制，
//! 默认关闭；当 stdout 是终端且未设置 `NO_COLOR` 时自动启用。

use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};

/// Prefix of ANSI CSI sequences.
/// ANSI CSI 序列的前缀。
const CSI: &str = "\x1b[";
/// Sequence that restores the default style.
/// 恢复默认样式的序列。
const RESET: &str = "\x1b[0m";

const BOLD: &str = "1";
const DIM: &str = "2";
const ITALIC: &str = "3";
const UNDERLINE: &str = "4";
const STRIKE: &str = "9";
const RED: &str = "31";
const GREEN: &str = "32";
const YELLOW: &str = "33";
const BLUE: &str = "34";
const MAGENTA: &str = "35";
const CYAN: &str = "36";
const GRAY: &str = "90";

/// Process-wide styling switch; disabled until explicitly enabled.
/// 进程级样式开关；显式启用前保持关闭。
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Turn styling on or off for the whole process.
/// 开启或关闭整个进程的样式。
pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// Whether styling is currently enabled.
/// 当前样式是否开启。
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Enable styling based on the environment: stdout is a terminal and
/// `NO_COLOR` is unset. Returns whether styling ended up enabled.
/// 根据环境启用样式：stdout 是终端且未设置 `NO_COLOR`。返回样式是否已启用。
pub fn auto_init() -> bool {
    auto_init_with(io::stdout().is_terminal())
}

/// Decide styling based on an explicit terminal flag, mirroring
/// `auto_init()` without consulting real stdout so tests are deterministic
/// regardless of the test harness TTY state.
fn auto_init_with(stdout_is_terminal: bool) -> bool {
    let enabled = should_enable(stdout_is_terminal);
    set_enabled(enabled);
    enabled
}

/// Wrap `text` in ANSI SGR `codes` followed by a reset, only when enabled.
/// 仅在启用时用 ANSI SGR `codes` 包裹 `text` 并追加重置序列。
fn apply(text: &str, codes: &[&str]) -> String {
    if !is_enabled() {
        return text.to_string();
    }
    format!("{CSI}{}m{text}{RESET}", codes.join(";"))
}

/// Bold text.
/// 加粗文本。
pub fn bold(text: &str) -> String {
    apply(text, &[BOLD])
}

/// Dim text.
/// 弱化文本。
pub fn dim(text: &str) -> String {
    apply(text, &[DIM])
}

/// Italic text.
/// 斜体文本。
pub fn italic(text: &str) -> String {
    apply(text, &[ITALIC])
}

/// Underlined text.
/// 下划线文本。
pub fn underline(text: &str) -> String {
    apply(text, &[UNDERLINE])
}

/// Strikethrough text.
/// 删除线文本。
pub fn strike(text: &str) -> String {
    apply(text, &[STRIKE])
}

/// Red text.
/// 红色文本。
pub fn red(text: &str) -> String {
    apply(text, &[RED])
}

/// Green text.
/// 绿色文本。
pub fn green(text: &str) -> String {
    apply(text, &[GREEN])
}

/// Yellow text.
/// 黄色文本。
pub fn yellow(text: &str) -> String {
    apply(text, &[YELLOW])
}

/// Blue text.
/// 蓝色文本。
pub fn blue(text: &str) -> String {
    apply(text, &[BLUE])
}

/// Magenta text.
/// 品红色文本。
pub fn magenta(text: &str) -> String {
    apply(text, &[MAGENTA])
}

/// Cyan text.
/// 青色文本。
pub fn cyan(text: &str) -> String {
    apply(text, &[CYAN])
}

/// Gray text.
/// 灰色文本。
pub fn gray(text: &str) -> String {
    apply(text, &[GRAY])
}

/// Section header text: bold cyan.
/// 区块标题文本：加粗青色。
pub fn header(text: &str) -> String {
    apply(text, &[BOLD, CYAN])
}

/// Secondary text: gray.
/// 次要文本：灰色。
pub fn muted(text: &str) -> String {
    apply(text, &[GRAY])
}

/// Success text: green.
/// 成功文本：绿色。
pub fn success(text: &str) -> String {
    apply(text, &[GREEN])
}

/// Error text: bold red.
/// 错误文本：加粗红色。
pub fn error(text: &str) -> String {
    apply(text, &[BOLD, RED])
}

/// Accent text: cyan.
/// 强调文本：青色。
pub fn accent(text: &str) -> String {
    apply(text, &[CYAN])
}

/// Remove ANSI CSI escape sequences (`ESC [ ... final byte`) from `text`.
/// A standalone ESC drops the ESC itself but keeps the following character.
/// 从 `text` 中移除 ANSI CSI 转义序列（`ESC [ ... 终结字节`）。
/// 单独的 ESC 会丢弃 ESC 本身，但保留其后一个字符。
pub fn strip_ansi(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            plain.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&c) {
                        break;
                    }
                }
            }
            Some(next) => plain.push(next),
            None => {}
        }
    }
    plain
}

/// Whether styling should be enabled for the given terminal state.
/// 给定终端状态时是否应启用样式。
fn should_enable(stdout_is_terminal: bool) -> bool {
    stdout_is_terminal && !no_color_set()
}

/// Whether the `NO_COLOR` environment variable is set.
/// `NO_COLOR` 环境变量是否已设置。
fn no_color_set() -> bool {
    std::env::var_os("NO_COLOR").is_some()
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    /// Serializes tests that mutate process-wide style state or env vars.
    /// 串行化修改进程级样式状态或环境变量的测试。
    static STYLE_LOCK: Mutex<()> = Mutex::new(());

    /// Lock for style tests; tolerates a poisoned mutex so one failing test
    /// does not hide the others.
    fn style_guard() -> MutexGuard<'static, ()> {
        STYLE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn should_enable_requires_terminal_without_no_color() {
        let _guard = style_guard();
        // SAFETY: guarded by STYLE_LOCK; no other test reads NO_COLOR concurrently.
        unsafe { std::env::remove_var("NO_COLOR") };
        assert!(should_enable(true));
        assert!(!should_enable(false));
    }

    #[test]
    fn no_color_disables_styling_even_on_terminal() {
        let _guard = style_guard();
        // SAFETY: guarded by STYLE_LOCK; no other test reads NO_COLOR concurrently.
        unsafe { std::env::set_var("NO_COLOR", "1") };
        assert!(!should_enable(true));
        // SAFETY: restore the environment for other tests.
        unsafe { std::env::remove_var("NO_COLOR") };
    }

    #[test]
    fn auto_init_disables_when_stdout_is_not_a_terminal() {
        let _guard = style_guard();
        // SAFETY: guarded by STYLE_LOCK; no other test reads NO_COLOR concurrently.
        unsafe { std::env::remove_var("NO_COLOR") };
        assert!(!auto_init_with(false));
        assert!(!is_enabled());
    }

    #[test]
    fn auto_init_enables_when_stdout_is_a_terminal_without_no_color() {
        let _guard = style_guard();
        // SAFETY: guarded by STYLE_LOCK; no other test reads NO_COLOR concurrently.
        unsafe { std::env::remove_var("NO_COLOR") };
        assert!(auto_init_with(true));
        assert!(is_enabled());
        set_enabled(false);
    }
}
