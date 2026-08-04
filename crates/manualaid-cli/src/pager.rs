//! # Description
//! Minimal console pager for long CLI output: prints everything when stdout
//! or stdin is not a terminal, or when the output fits on one screen;
//! otherwise shows one screenful per key press (`q` quits early).
//! # 描述
//! 用于长 CLI 输出的最小控制台分页器：stdout 或 stdin 非终端、或输出
//! 一屏放得下时全量输出；否则每按一次键显示一屏（按 `q` 提前退出）。

use std::io::{self, IsTerminal, Write};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::style;

/// # Description
/// Print `output`, paging it when it is longer than the terminal height.
/// # 描述
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
    use crossterm::cursor::MoveToColumn;
    use crossterm::event::KeyModifiers;
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{Clear, ClearType};

    let _raw = RawModeGuard::enable()?;
    let mut stdout = io::stdout().lock();
    let mut start = 0usize;
    while start < lines.len() {
        let end = (start + page_size).min(lines.len());
        write_page(&mut stdout, lines, start, end)?;
        start = end;
        if start >= lines.len() {
            break;
        }
        write!(stdout, "{}", style::muted(&i18n::t_str("cli.pager.more")))?;
        stdout.flush()?;
        let quit = match event::read()? {
            Event::Key(key) => {
                matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
            }
            _ => false,
        };
        crossterm::execute!(stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        if quit {
            break;
        }
    }
    stdout.flush()
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
}
