//! Minimal console pager for long CLI output: prints everything when stdout
//! or stdin is not a terminal, or when the output fits on one screen;
//! otherwise shows one screenful per key press (`q` quits early).
//! A collapsed variant starts with a fixed three-line preview so huge tool
//! results do not flood the console on the first key press.
//! 用于长 CLI 输出的最小控制台分页器：stdout 或 stdin 非终端、或输出
//! 一屏放得下时全量输出；否则每按一次键显示一屏（按 `q` 提前退出）。
//! 折叠变体先固定显示 3 行预览，避免工具结果一次刷过整屏。

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::style;

/// Whether interactive paging is allowed. Disabled in test builds so tests
/// never enter raw mode or block on key presses; the switch stays available
/// for integration tests that link the non-test library.
/// 是否允许交互分页。测试构建下关闭，测试永不进入 raw mode 或阻塞等按键；
/// 集成测试链接的是非 test 库，因此保留显式开关。
static INTERACTIVE_ENABLED: AtomicBool = AtomicBool::new(!cfg!(test));

/// Turn interactive paging on or off for the whole process.
/// 开关整个进程的交互分页。
pub fn set_enabled(enabled: bool) {
    INTERACTIVE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Print `output`, paging it when it is longer than the terminal height.
/// 输出 `output`，当内容超过终端高度时分页显示。
pub fn print_paged(output: &str) -> io::Result<()> {
    if crate::console::is_capturing() {
        return print_all_to(output, &mut crate::console::ConsoleWriter);
    }
    if !INTERACTIVE_ENABLED.load(Ordering::Relaxed) {
        return print_all(output);
    }
    print_paged_with(
        output,
        None,
        io::stdout().is_terminal(),
        io::stdin().is_terminal(),
        terminal_height(),
        |lines, first_page_size, page_size| {
            interactive_paged(io::stdout(), lines, first_page_size, page_size, read_key)
        },
    )
}

/// Number of lines shown on the first collapsed page.
/// 折叠分页首页固定显示的行数。
const COLLAPSED_FIRST_PAGE_LINES: usize = 3;

/// Print `output`, starting with a three-line preview and then showing one
/// full screen per key press when it is longer; `q`/Ctrl+C quits early.
/// 输出 `output`：超过 3 行时先显示固定预览，之后每按一次键显示一整屏，
/// `q`/Ctrl+C 可提前退出。
pub fn print_paged_collapsed(output: &str) -> io::Result<()> {
    if crate::console::is_capturing() {
        return print_all_to(output, &mut crate::console::ConsoleWriter);
    }
    if !INTERACTIVE_ENABLED.load(Ordering::Relaxed) {
        return print_all(output);
    }
    print_paged_with(
        output,
        Some(COLLAPSED_FIRST_PAGE_LINES),
        io::stdout().is_terminal(),
        io::stdin().is_terminal(),
        terminal_height(),
        |lines, first_page_size, page_size| {
            interactive_paged(io::stdout(), lines, first_page_size, page_size, read_key)
        },
    )
}

/// Write `output` to `writer`, appending a newline when it does not already
/// end with one so the next prompt or menu starts on a fresh line.
/// 把 `output` 写入 `writer`；若输出未以换行结尾则补一个换行，避免后续
/// 提示或菜单紧贴末行。
fn print_all_to(output: &str, writer: &mut impl Write) -> io::Result<()> {
    writer.write_all(output.as_bytes())?;
    if !output.ends_with('\n') {
        writer.write_all(b"\n")?;
    }
    writer.flush()
}

/// Write the whole output without paging; the sink honors an active test
/// capture, so non-interactive output never reaches the real terminal in
/// tests.
/// 不分页地写入完整输出；出口尊重测试捕获状态，非交互输出在测试中不会
/// 到达真实终端。
fn print_all(output: &str) -> io::Result<()> {
    print_all_to(output, &mut crate::console::ConsoleWriter)
}

/// Decide whether to print everything or start paging, based on injectable
/// terminal state so unit tests can cover every branch without a console.
/// 根据可注入的终端状态决定全量输出还是分页，使单元测试无需控制台即可
/// 覆盖每个分支。
fn print_paged_with<F>(
    output: &str,
    first_page_size: Option<usize>,
    stdout_is_terminal: bool,
    stdin_is_terminal: bool,
    height: Option<usize>,
    run_interactive: F,
) -> io::Result<()>
where
    F: FnOnce(&[&str], usize, usize) -> io::Result<()>,
{
    if !stdout_is_terminal || !stdin_is_terminal {
        return print_all(output);
    }
    let lines: Vec<&str> = output.lines().collect();
    let page_size = page_size_for(height.unwrap_or(24));
    let first_page = first_page_size.unwrap_or(page_size);
    if lines.len() <= first_page {
        return print_all(output);
    }
    run_interactive(&lines, first_page, page_size)
}

/// Enable raw mode and page the lines to the real stdout.
/// 启用 raw mode 并把内容分页输出到真实 stdout。
fn interactive_paged<W, F>(
    writer: W,
    lines: &[&str],
    first_page_size: usize,
    page_size: usize,
    read_key: F,
) -> io::Result<()>
where
    W: Write,
    F: FnMut() -> io::Result<bool>,
{
    let _raw = RawModeGuard::enable()?;
    write_pages_with(writer, lines, first_page_size, page_size, read_key)
}

/// Write the paged lines to an injectable writer; the writer is flushed
/// after the final page so callers see the output immediately.
/// 把分页内容写入可注入的 writer；最后一页写完后 flush，调用方立即可见。
fn write_pages_with<W, F>(
    mut writer: W,
    lines: &[&str],
    first_page_size: usize,
    page_size: usize,
    read_key: F,
) -> io::Result<()>
where
    W: Write,
    F: FnMut() -> io::Result<bool>,
{
    run_pages_collapsed(&mut writer, lines, first_page_size, page_size, read_key)?;
    writer.flush()
}

/// Read one key and decide between quitting early and continuing.
/// 读取一次按键并判断是否提前退出。
fn read_key() -> io::Result<bool> {
    use crossterm::event::{self, Event};
    match event::read()? {
        Event::Key(key) => Ok(is_quit_key(key)),
        _ => Ok(false),
    }
}

/// Drive a collapsed paging loop: `first_page_size` lines first, then one
/// full screen per iteration. Pure console logic, so it is unit-testable
/// without a terminal.
/// 驱动折叠分页循环：先显示 `first_page_size` 行，后续每次迭代显示一整屏。
/// 循环本身是纯控制台逻辑，无需终端即可单元测试。
fn run_pages_collapsed<F>(
    stdout: &mut impl Write,
    lines: &[&str],
    first_page_size: usize,
    page_size: usize,
    read_key: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<bool>,
{
    run_pages_with_steps(
        stdout,
        lines,
        page_steps_with_first(lines.len(), first_page_size, page_size),
        read_key,
    )
}

/// Drive the shared paging loop over precomputed page bounds; `read_key`
/// decides between quitting early and showing the next page.
/// 在页面边界序列上驱动分页循环；`read_key` 决定提前退出还是显示下一页。
fn run_pages_with_steps<F>(
    stdout: &mut impl Write,
    lines: &[&str],
    steps: impl Iterator<Item = (usize, usize)>,
    mut read_key: F,
) -> io::Result<()>
where
    F: FnMut() -> io::Result<bool>,
{
    use crossterm::cursor::MoveToColumn;
    use crossterm::terminal::{Clear, ClearType};

    for (start, end) in steps {
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

/// The `(start, end)` bounds of a collapsed paging sequence: one small
/// first page followed by full pages clipped to the total line count.
/// 折叠分页的 `(start, end)` 边界：一小页后接整屏，最后一页裁剪到总行数。
fn page_steps_with_first(
    total: usize,
    first_page_size: usize,
    page_size: usize,
) -> impl Iterator<Item = (usize, usize)> {
    let first_page_size = first_page_size.max(1);
    let page_size = page_size.max(1);
    let first_end = first_page_size.min(total);
    std::iter::once((0, first_end))
        .chain(
            (first_end..total)
                .step_by(page_size)
                .map(move |start| (start, (start + page_size).min(total))),
        )
        .take_while(|&(start, end)| start < end)
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

/// Write the lines in `lines[start..end]`, one per line, each terminated
/// by CRLF. Raw mode disables the terminal's output post-processing, so a
/// bare LF would move the cursor down without returning it to column 0 and
/// every paged line would start where the previous one ended.
/// 逐行写出 `lines[start..end]`，每行以 CRLF 结尾。raw mode 会关闭终端
/// 输出后处理，若只写 LF，光标只会下移而不会回到第 0 列，分页内容会
/// 逐行向右错位。
fn write_page(stdout: &mut impl Write, lines: &[&str], start: usize, end: usize) -> io::Result<()> {
    for line in &lines[start..end] {
        write!(stdout, "{line}\r\n")?;
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

    /// Serializes tests that flip the process-wide interactive-pager switch.
    /// 串行化切换进程级交互分页开关的测试。
    static PAGER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Interactive runner that must never be invoked by the branch under
    /// test; kept as one shared function so it is not duplicated per test.
    /// 被测分支不应触发的交互运行器；定义为共享函数避免在每个测试里重复。
    fn interactive_unexpected(
        _lines: &[&str],
        _first_page_size: usize,
        _page_size: usize,
    ) -> io::Result<()> {
        unreachable!("interactive paging must not run")
    }

    /// Key reader that must never be invoked when the output fits one page.
    /// 输出一页放得下时不应被调用的按键读取器。
    fn key_unexpected() -> io::Result<bool> {
        unreachable!("pager must not read a key")
    }

    #[test]
    fn page_size_reserves_one_line_for_the_prompt() {
        assert_eq!(page_size_for(24), 23);
        assert_eq!(page_size_for(1), 1);
        assert_eq!(page_size_for(0), 1);
    }

    #[test]
    fn terminal_height_is_positive_when_available_or_missing_otherwise() {
        if let Some(height) = terminal_height() {
            assert!(height > 0);
        }
    }

    #[test]
    fn print_paged_with_prints_all_without_a_terminal() {
        let _capture = crate::console::capture();
        assert!(
            print_paged_with(
                "a\nb\n",
                None,
                false,
                true,
                Some(24),
                interactive_unexpected
            )
            .is_ok()
        );
        assert!(
            print_paged_with(
                "a\nb\n",
                None,
                true,
                false,
                Some(24),
                interactive_unexpected
            )
            .is_ok()
        );
    }

    #[test]
    fn print_paged_prints_all_when_interactive_is_disabled() {
        let _lock = PAGER_LOCK.lock().unwrap();
        let capture = crate::console::capture();
        set_enabled(false);
        assert!(print_paged("a\nb\nc\n").is_ok());
        assert!(print_paged_collapsed("a\nb\nc\nd\n").is_ok());
        set_enabled(!cfg!(test));
        assert_eq!(capture.text(), "a\nb\nc\na\nb\nc\nd\n");
    }

    #[test]
    fn print_paged_prints_with_interactive_enabled_when_output_fits() {
        let _lock = PAGER_LOCK.lock().unwrap();
        let capture = crate::console::capture();
        set_enabled(true);
        assert!(print_paged("a\nb\n").is_ok());
        assert!(print_paged_collapsed("a\nb\nc\n").is_ok());
        set_enabled(!cfg!(test));
        assert_eq!(capture.text(), "a\nb\na\nb\nc\n");
    }

    #[test]
    fn print_all_to_appends_a_newline_only_when_missing() {
        let mut out = Vec::new();
        print_all_to("a\nb\n", &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "a\nb\n");
        let mut out = Vec::new();
        print_all_to("a\nb", &mut out).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "a\nb\n");
    }

    #[test]
    fn print_paged_with_prints_all_when_output_fits_the_first_page() {
        let _capture = crate::console::capture();
        assert!(
            print_paged_with(
                "a\nb\nc\n",
                Some(3),
                true,
                true,
                Some(24),
                interactive_unexpected
            )
            .is_ok()
        );
        assert!(
            print_paged_with("a\nb\n", None, true, true, Some(24), interactive_unexpected).is_ok()
        );
    }

    #[test]
    fn print_paged_with_delegates_to_interactive_paging_for_long_output() {
        let mut calls = 0;
        let result = print_paged_with(
            "a\nb\nc\nd\ne\nf\n",
            Some(3),
            true,
            true,
            Some(24),
            |lines, first_page_size, page_size| {
                calls += 1;
                assert_eq!(lines, ["a", "b", "c", "d", "e", "f"]);
                assert_eq!(first_page_size, 3);
                assert_eq!(page_size, 23);
                Ok(())
            },
        );
        assert_eq!(calls, 1);
        assert!(result.is_ok());
    }

    #[test]
    fn collapsed_pages_quit_after_the_first_page_without_stdout_pollution() {
        let mut out = Vec::new();
        run_pages_collapsed(&mut out, &["a", "b", "c", "d"], 3, 4, || Ok(true)).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("a\r\nb\r\nc\r\n"));
    }

    #[test]
    fn interactive_paged_quits_immediately_when_raw_mode_is_available() {
        // The injected key reader quits at once and the writer is an
        // in-memory buffer, so the test never blocks and never writes to
        // the real console; without a console raw mode fails and the pager
        // returns an error.
        // 注入的按键读取器立即退出且 writer 是内存缓冲，测试不会阻塞也不会
        // 写真实控制台；无控制台时 raw mode 失败，分页器返回错误。
        let mut out = Vec::new();
        if let Ok(()) = interactive_paged(&mut out, &["a", "b", "c", "d"], 3, 4, || Ok(true)) {
            let text = String::from_utf8(out).unwrap();
            assert!(text.starts_with("a\r\nb\r\nc\r\n"));
        }
    }

    #[test]
    fn write_page_writes_the_requested_slice() {
        let lines = ["a", "b", "c", "d"];
        let mut out = Vec::new();
        write_page(&mut out, &lines, 1, 3).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "b\r\nc\r\n");
    }

    #[test]
    fn write_page_handles_an_empty_slice() {
        let lines = ["a", "b"];
        let mut out = Vec::new();
        write_page(&mut out, &lines, 1, 1).unwrap();
        assert_eq!(out, b"");
    }

    #[test]
    fn full_page_steps_are_collapsed_with_equal_first_and_page_size() {
        let steps: Vec<(usize, usize)> = page_steps_with_first(100, 23, 23).collect();
        assert_eq!(steps, [(0, 23), (23, 46), (46, 69), (69, 92), (92, 100)]);
        assert_eq!(
            page_steps_with_first(4, 0, 0).collect::<Vec<_>>(),
            [(0, 1), (1, 2), (2, 3), (3, 4)]
        );
    }

    #[test]
    fn collapsed_page_steps_start_with_three_lines_then_full_pages() {
        let steps: Vec<(usize, usize)> = page_steps_with_first(10, 3, 4).collect();
        assert_eq!(steps, [(0, 3), (3, 7), (7, 10)]);
    }

    #[test]
    fn collapsed_page_steps_clip_to_total_and_handle_small_inputs() {
        assert_eq!(page_steps_with_first(2, 3, 4).collect::<Vec<_>>(), [(0, 2)]);
        assert_eq!(
            page_steps_with_first(5, 0, 2).collect::<Vec<_>>(),
            [(0, 1), (1, 3), (3, 5)]
        );
        assert_eq!(page_steps_with_first(0, 3, 4).collect::<Vec<_>>(), []);
    }

    #[test]
    fn collapsed_pages_ask_for_a_key_between_first_page_and_the_rest() {
        let lines = ["a", "b", "c", "d", "e", "f", "g", "h"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages_collapsed(&mut out, &lines, 3, 4, || {
            key_reads += 1;
            Ok(false)
        })
        .unwrap();
        assert_eq!(key_reads, 2);
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("a\r\nb\r\nc\r\n"));
        assert!(text.contains("d\r\ne\r\nf\r\ng\r\n"));
        assert!(text.ends_with("h\r\n"));
    }

    #[test]
    fn collapsed_pages_quit_after_the_first_page_on_quit_key() {
        let lines = ["a", "b", "c", "d", "e", "f"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages_collapsed(&mut out, &lines, 3, 4, || {
            key_reads += 1;
            Ok(true)
        })
        .unwrap();
        assert_eq!(key_reads, 1);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("a\r\nb\r\nc\r\n"));
        assert!(!text.contains("d\r\n"));
    }

    #[test]
    fn collapsed_pages_ask_no_key_when_output_fits_the_first_page() {
        let mut out = Vec::new();
        run_pages_collapsed(&mut out, &["a", "b"], 3, 4, key_unexpected).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "a\r\nb\r\n");
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
    fn full_pages_ask_for_a_key_between_pages() {
        let lines = ["a", "b", "c", "d", "e"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages_collapsed(&mut out, &lines, 4, 4, || {
            key_reads += 1;
            Ok(false)
        })
        .unwrap();
        assert_eq!(key_reads, 1);
        let text = String::from_utf8(out).unwrap();
        // Between the pages the pager writes the "more" prompt and a clear
        // sequence, so only the page content itself is asserted.
        // 页与页之间分页器会写入 "more" 提示与清除序列，因此只断言页面内容。
        assert!(text.starts_with("a\r\nb\r\nc\r\nd\r\n"));
        assert!(text.ends_with("e\r\n"));
    }

    #[test]
    fn full_pages_quit_after_the_first_page_on_quit_key() {
        let lines = ["a", "b", "c", "d", "e", "f"];
        let mut out = Vec::new();
        let mut key_reads = 0;
        run_pages_collapsed(&mut out, &lines, 4, 4, || {
            key_reads += 1;
            Ok(true)
        })
        .unwrap();
        assert_eq!(key_reads, 1);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("a\r\nb\r\nc\r\nd\r\n"));
        assert!(!text.contains("e\r\n"));
    }

    #[test]
    fn full_pages_ask_no_key_when_output_fits_one_page() {
        let mut out = Vec::new();
        run_pages_collapsed(&mut out, &["only"], 4, 4, key_unexpected).unwrap();
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
