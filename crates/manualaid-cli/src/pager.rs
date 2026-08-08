//! Minimal console pager for long CLI output: prints everything when stdout
//! or stdin is not a terminal, or when the output fits on one screen;
//! otherwise shows one screenful per key press (`q` quits early).
//! 用于长 CLI 输出的最小控制台分页器：stdout 或 stdin 非终端、或输出
//! 一屏放得下时全量输出；否则每按一次键显示一屏（按 `q` 提前退出）。

use std::io::{self, IsTerminal, Write};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::style;

/// Print `output`, paging it when it is longer than the terminal height.
/// 输出 `output`，当内容超过终端高度时分页显示。
pub fn print_paged(output: &str) -> io::Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        return print_all(output);
    }
    let lines: Vec<&str> = output.lines().collect();
    let page_size = page_size_for(terminal_height().unwrap_or(24));
    if lines.len() <= page_size {
        return print_all(output);
    }
    interactive_page(&lines, page_size)
}

/// Write the whole output without paging.
/// 不分页地写入完整输出。
fn print_all(output: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(output.as_bytes())?;
    stdout.flush()
}

/// Show one screenful per key press; `q`/`Q`/Ctrl+C quits early, any other
/// key shows the next page.
/// 每按一次键显示一屏；`q`/`Q`/Ctrl+C 提前退出，按其他键显示下一页。
fn interactive_page(lines: &[&str], page_size: usize) -> io::Result<()> {
    let _raw = RawModeGuard::enable()?;
    let mut stdout = io::stdout().lock();
    run_pages(&mut stdout, lines, page_size, || {
        use crossterm::event::{self, Event};
        match event::read()? {
            Event::Key(key) => Ok(is_quit_key(key)),
            _ => Ok(false),
        }
    })?;
    stdout.flush()
}

/// Drive the paging loop: one screenful per iteration; `read_key` decides
/// between quitting early and showing the next page. Pure console logic, so
/// it is unit-testable without a terminal.
/// 驱动分页循环：每次迭代显示一屏；`read_key` 决定提前退出还是显示下一页。
/// 循环本身是纯控制台逻辑，无需终端即可单元测试。
fn run_pages<F>(
    stdout: &mut impl Write,
    lines: &[&str],
    page_size: usize,
    mut read_key: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<bool>,
{
    use crossterm::cursor::MoveToColumn;
    use crossterm::terminal::{Clear, ClearType};

    for (start, end) in page_steps(lines.len(), page_size) {
        write_page(stdout, lines, start, end)?;
        if end >= lines.len() {
            break;
        }
        write!(stdout, "{}", style::muted(&i18n::t_str("cli.pager.more")))?;
        stdout.flush()?;
        let quit = read_key()?;
        crossterm::execute!(stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        if quit {
            break;
        }
    }
    Ok(())
}

/// The `(start, end)` bounds of each page; the last page is clipped to the
/// total line count. Pure logic so the paging loop is testable without a
/// terminal.
/// 每页的 `(start, end)` 边界；最后一页裁剪到总行数。纯逻辑，使分页循环
/// 无需终端即可测试。
fn page_steps(total: usize, page_size: usize) -> impl Iterator<Item = (usize, usize)> {
    let page_size = page_size.max(1);
    (0..total)
        .step_by(page_size)
        .map(move |start| (start, (start + page_size).min(total)))
}

/// Whether a key press should quit the pager early.
/// 某次按键是否应提前退出分页器。
fn is_quit_key(key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyModifiers;
    matches!(
        key.code,
        crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Char('Q')
    ) || (key.code == crossterm::event::KeyCode::Char('c')
        && key.modifiers.contains(KeyModifiers::CONTROL))
}

/// Write the lines in `lines[start..end]`, one per line.
/// 逐行写出 `lines[start..end]`。
fn write_page(stdout: &mut impl Write, lines: &[&str], start: usize, end: usize) -> io::Result<()> {
    for line in &lines[start..end] {
        writeln!(stdout, "{line}")?;
    }
    Ok(())
}

/// One page: the terminal height minus one line reserved for the prompt.
/// 每页行数：终端高度减去留给提示行的一行。
fn page_size_for(height: usize) -> usize {
    height.saturating_sub(1).max(1)
}

/// Restores the terminal raw mode on drop.
/// Drop 时恢复终端的 raw mode。
struct RawModeGuard;

impl RawModeGuard {
    /// Enable raw mode, returning a guard that restores it on drop.
    /// 启用 raw mode，返回在 Drop 时恢复它的守卫。
    fn enable() -> io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

/// Current terminal height in rows, if it can be queried.
/// 当前终端行数（可查询时）。
fn terminal_height() -> Option<usize> {
    crossterm::terminal::size()
        .ok()
        .map(|(_, rows)| rows as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_reserves_one_line_for_the_prompt() {
        assert_eq!(page_size_for(24), 23);
        assert_eq!(page_size_for(1), 1);
        assert_eq!(page_size_for(0), 1);
    }

    #[test]
    fn terminal_height_is_positive_when_available() {
        if let Some(height) = terminal_height() {
            assert!(height > 0);
        }
    }

    #[test]
    fn write_page_writes_the_requested_slice() {
        let lines = ["a", "b", "c", "d"];
        let mut out = Vec::new();
        write_page(&mut out, &lines, 1, 3).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "b\nc\n");
    }

    #[test]
    fn write_page_handles_an_empty_slice() {
        let lines = ["a", "b"];
        let mut out = Vec::new();
        write_page(&mut out, &lines, 1, 1).unwrap();
        assert_eq!(out, b"");
    }

    #[test]
    fn page_steps_walks_pages_and_clips_the_last() {
        let steps: Vec<(usize, usize)> = page_steps(100, 23).collect();
        assert_eq!(steps, [(0, 23), (23, 46), (46, 69), (69, 92), (92, 100)]);
    }

    #[test]
    fn page_steps_handles_small_and_empty_inputs() {
        assert_eq!(page_steps(5, 10).collect::<Vec<_>>(), [(0, 5)]);
        assert_eq!(page_steps(0, 10).collect::<Vec<_>>(), []);
        assert_eq!(
            page_steps(4, 0).collect::<Vec<_>>(),
            [(0, 1), (1, 2), (2, 3), (3, 4)]
        );
    }

    #[test]
    fn quit_keys_are_q_and_ctrl_c() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        assert!(is_quit_key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE
        )));
        assert!(is_quit_key(KeyEvent::new(
            KeyCode::Char('Q'),
            KeyModifiers::NONE
        )));
        assert!(is_quit_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL
        )));
        assert!(!is_quit_key(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::NONE
        )));
        assert!(!is_quit_key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::NONE
        )));
    }

    #[test]
    fn run_pages_writes_every_page_asking_for_a_key_between_pages() {
        let lines = ["a", "b", "c", "d", "e"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages(&mut out, &lines, 4, || {
            key_reads += 1;
            Ok(false)
        })
        .unwrap();
        assert_eq!(key_reads, 1);
        let text = String::from_utf8(out).unwrap();
        // Between the pages the pager writes the "more" prompt and a clear
        // sequence, so only the page content itself is asserted.
        // 页与页之间分页器会写入 "more" 提示与清除序列，因此只断言页面内容。
        assert!(text.starts_with("a\nb\nc\nd\n"));
        assert!(text.ends_with("e\n"));
    }

    #[test]
    fn run_pages_quits_after_the_first_page_on_quit_key() {
        let lines = ["a", "b", "c", "d", "e", "f"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages(&mut out, &lines, 4, || {
            key_reads += 1;
            Ok(true)
        })
        .unwrap();
        assert_eq!(key_reads, 1);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("a\nb\nc\nd\n"));
        assert!(!text.contains("e\n"));
    }

    #[test]
    fn run_pages_asks_no_key_when_the_output_fits_one_page() {
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages(&mut out, &["only"], 4, || {
            key_reads += 1;
            Ok(false)
        })
        .unwrap();
        assert_eq!(key_reads, 0);
    }

    #[test]
    fn raw_mode_guard_enables_and_restores_raw_mode_when_possible() {
        // Raw mode needs a console; when the test process has none the call
        // fails and the guard is simply not exercised.
        // raw mode 需要控制台；测试进程无控制台时调用失败，守卫不会被
        // 实际启用。
        if let Ok(guard) = RawModeGuard::enable() {
            drop(guard);
        }
    }
}
