//! Single-line progress display for the tool execution phase of one round:
//! while tools run, the cursor's current line shows `(Ns) TOOL1, TOOL2, ...`
//! with a per-tool state color (pending = gray, running = yellow, success =
//! green, failed = red); once the round's tools finish, the line is frozen
//! and terminated with a newline before the legacy round output is printed.
//! 单轮工具执行阶段的单行进度显示：工具运行时，光标当前行显示
//! `(Ns) TOOL1, TOOL2, ...`，每个工具按状态着色（未执行=灰、执行中=黄、
//! 成功=绿、失败=红）；本轮工具执行完毕后，该行定格并以换行结束，随后
//! 才输出旧版轮次信息。

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use crossterm::cursor::MoveToColumn;
use crossterm::terminal::{Clear, ClearType};

use crate::style;

/// Per-tool state shown in the progress line.
/// 进度行中单个工具的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolState {
    /// Not started yet: gray.
    /// 尚未开始：灰色。
    Pending,
    /// Currently executing: yellow.
    /// 正在执行：黄色。
    Running,
    /// Finished successfully: green.
    /// 执行成功：绿色。
    Success,
    /// Finished with a failure: red.
    /// 执行失败：红色。
    Failed,
}

/// One tool entry in the progress line.
/// 进度行中的一个工具条目。
struct ToolProgress {
    /// Tool name as parsed from the round.
    /// 从该轮解析出的工具名。
    name: String,
    /// Current state, driving the color.
    /// 当前状态，决定颜色。
    state: ToolState,
}

/// The execution-phase progress line. Every method is a no-op when the
/// line is disabled (test builds, non-terminal stdout or an active
/// capture), so tests and piped output never see progress escapes.
/// 执行阶段的进度行。禁用时（测试构建、非终端 stdout 或活动捕获）所有
/// 方法均为空操作，测试与管道输出因此绝不会看到进度转义序列。
pub(super) struct ProgressLine {
    /// Tools of the round, in execution order.
    /// 该轮的工具，按执行顺序排列。
    tools: Vec<ToolProgress>,
    /// Whether the line may be drawn on the real terminal.
    /// 是否可在真实终端上绘制该行。
    enabled: bool,
    /// Whether the line was drawn and has not been erased since; used to
    /// decide whether `finish` must emit the terminating newline.
    /// 该行是否已绘制且此后未被擦除；用于判断 `finish` 是否需补换行。
    drawn: bool,
    /// Start of the execution phase, driving the elapsed seconds.
    /// 执行阶段的起点，决定已过去的秒数。
    start: Instant,
    /// Terminal width in columns when it can be queried; `None` never
    /// truncates.
    /// 可查询时的终端列数；`None` 表示不截断。
    max_width: Option<usize>,
}

impl ProgressLine {
    /// Build a progress line for the given tool names, deciding once whether
    /// the line may be drawn on the real terminal.
    /// 为给定工具名构建进度行，一次性决定是否可在真实终端上绘制。
    pub(super) fn new(names: Vec<String>) -> Self {
        Self::with_options(names, should_enable(), terminal_width())
    }

    /// Build a progress line with explicit options so tests can exercise
    /// the drawing paths without a real terminal.
    /// 用显式选项构建进度行，使测试无需真实终端即可覆盖绘制路径。
    fn with_options(names: Vec<String>, enabled: bool, max_width: Option<usize>) -> Self {
        Self {
            tools: names
                .into_iter()
                .map(|name| ToolProgress {
                    name,
                    state: ToolState::Pending,
                })
                .collect(),
            enabled,
            drawn: false,
            start: Instant::now(),
            max_width,
        }
    }

    /// Mark the tool at `index` as running and redraw.
    /// 把第 `index` 个工具标记为执行中并重绘。
    pub(super) fn running(&mut self, index: usize) {
        self.set_state(index, ToolState::Running);
    }

    /// Mark the tool at `index` as succeeded and redraw.
    /// 把第 `index` 个工具标记为成功并重绘。
    pub(super) fn succeed(&mut self, index: usize) {
        self.set_state(index, ToolState::Success);
    }

    /// Mark the tool at `index` as failed and redraw.
    /// 把第 `index` 个工具标记为失败并重绘。
    pub(super) fn fail(&mut self, index: usize) {
        self.set_state(index, ToolState::Failed);
    }

    /// Refresh the elapsed seconds without changing any tool state.
    /// 仅刷新已过去的秒数，不改变任何工具状态。
    pub(super) fn tick(&mut self) {
        self.draw();
    }

    /// Erase the current line so other output can be written; the next draw
    /// repaints from column 0. A no-op when nothing was drawn.
    /// 擦除当前行以便写入其他内容；下一次绘制会从第 0 列重绘。未绘制过时
    /// 为空操作。
    pub(super) fn suspend(&mut self) {
        self.suspend_with(&mut std::io::stdout());
    }

    /// Erase the current line into an injectable writer; the enabled and
    /// drawn state is still enforced so tests can drive the branch without a
    /// real terminal.
    /// 把当前行擦除到可注入的 writer；仍会检查启用与已绘制状态，使测试无需
    /// 真实终端即可覆盖该分支。
    fn suspend_with(&mut self, writer: &mut impl Write) {
        if !self.enabled || !self.drawn {
            return;
        }
        let _ = erase_current_line(writer);
        self.drawn = false;
    }

    /// Freeze the line and terminate it with a newline so later output
    /// starts on a fresh line. Does nothing when nothing was drawn.
    /// 定格该行并以换行结束，使后续输出从新行开始。未绘制过时不做事。
    pub(super) fn finish(&mut self) {
        self.finish_with(&mut std::io::stdout());
    }

    /// Terminate the line into an injectable writer; the enabled check stays
    /// here so tests can exercise the branch without a real terminal.
    /// 把该行收尾到可注入的 writer；启用检查保留在此，使测试无需真实终端
    /// 即可覆盖该分支。
    fn finish_with(&mut self, writer: &mut impl Write) {
        if !self.enabled {
            return;
        }
        let _ = self.finish_to(writer);
    }

    /// Update one tool's state and repaint.
    /// 更新单个工具的状态并重绘。
    fn set_state(&mut self, index: usize, state: ToolState) {
        if let Some(tool) = self.tools.get_mut(index) {
            tool.state = state;
        }
        self.draw();
    }

    /// Repaint the line on the real stdout, recording whether it succeeded.
    /// 在真实 stdout 上重绘该行，并记录是否成功。
    fn draw(&mut self) {
        self.draw_with(&mut std::io::stdout());
    }

    /// Repaint into an injectable writer, recording whether the draw
    /// succeeded; the enabled check stays here so tests can exercise the
    /// branch without a real terminal.
    /// 重绘到可注入的 writer 并记录是否成功；启用检查保留在此，使测试无需
    /// 真实终端即可覆盖该分支。
    fn draw_with(&mut self, writer: &mut impl Write) {
        if !self.enabled {
            return;
        }
        if self.draw_to(writer).is_ok() {
            self.drawn = true;
        }
    }

    /// Write the current line into an injectable writer: erase the current
    /// line, write the rendered text without a newline, then flush.
    /// 把当前行写入可注入的 writer：擦除当前行、写入渲染文本（不加换行）、
    /// 然后 flush。
    fn draw_to(&self, writer: &mut impl Write) -> std::io::Result<()> {
        let text = render(self.start.elapsed(), &self.tools, self.max_width);
        erase_current_line(writer)?;
        writer.write_all(text.as_bytes())?;
        writer.flush()
    }

    /// Redraw the frozen line and terminate it with a newline; a no-op when
    /// nothing was drawn.
    /// 重绘定格行并以换行结束；未绘制过时为空操作。
    fn finish_to(&mut self, writer: &mut impl Write) -> std::io::Result<()> {
        if !self.drawn {
            return Ok(());
        }
        self.draw_to(writer)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        self.drawn = false;
        Ok(())
    }
}

/// Erase the cursor's current line and move to column 0.
/// 擦除光标当前行并移到第 0 列。
fn erase_current_line(writer: &mut impl Write) -> std::io::Result<()> {
    crossterm::execute!(writer, MoveToColumn(0), Clear(ClearType::CurrentLine))
}

/// Render the whole line: `(Ns) T1, T2, ...`, colored per tool state and
/// truncated to `max_width` display columns when given.
/// 渲染整行：`(Ns) T1, T2, ...`，按工具状态着色，给定 `max_width` 时截断到
/// 对应显示列数。
fn render(elapsed: Duration, tools: &[ToolProgress], max_width: Option<usize>) -> String {
    let prefix = format!("({}s) ", elapsed.as_secs());
    let plain = tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let plain_line = format!("{prefix}{plain}");
    if let Some(width) = max_width
        && width > 0
        && plain_line.chars().count() > width
    {
        // Truncation drops the per-tool colors: slicing inside an ANSI
        // sequence would corrupt the escape stream, and a truncated line is
        // an edge case that does not need styling.
        // 截断会丢失各工具颜色：在 ANSI 序列中间截断会破坏转义流，且截断
        // 本身属于边缘情况，无需着色。
        let truncated: String = plain_line.chars().take(width.saturating_sub(1)).collect();
        return format!("{truncated}…");
    }
    let colored = tools.iter().map(style_tool).collect::<Vec<_>>().join(", ");
    format!("{prefix}{colored}")
}

/// Color one tool name according to its state.
/// 按状态为一个工具名着色。
fn style_tool(tool: &ToolProgress) -> String {
    match tool.state {
        ToolState::Pending => style::gray(&tool.name),
        ToolState::Running => style::yellow(&tool.name),
        ToolState::Success => style::green(&tool.name),
        ToolState::Failed => style::red(&tool.name),
    }
}

/// Whether the progress line may be drawn: never in test builds, never while
/// a console capture is active, and only when stdout is a real terminal.
/// 是否可绘制进度行：测试构建下绝不绘制，控制台捕获期间绝不绘制，且仅在
/// stdout 是真实终端时绘制。
fn should_enable() -> bool {
    !cfg!(test) && !crate::console::is_capturing() && std::io::stdout().is_terminal()
}

/// Current terminal width in columns, if it can be queried.
/// 当前终端列数（可查询时）。
fn terminal_width() -> Option<usize> {
    crossterm::terminal::size()
        .ok()
        .map(|(cols, _)| cols as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build tool entries with the given states and simple names.
    /// 按给定状态构建工具条目，使用简单名称。
    fn tools_with(states: &[ToolState]) -> Vec<ToolProgress> {
        states
            .iter()
            .enumerate()
            .map(|(index, state)| ToolProgress {
                name: format!("tool{index}"),
                state: *state,
            })
            .collect()
    }

    #[test]
    fn render_colors_each_state_when_style_is_enabled() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(true);
        let tools = tools_with(&[
            ToolState::Pending,
            ToolState::Running,
            ToolState::Success,
            ToolState::Failed,
        ]);
        let text = render(Duration::from_secs(0), &tools, None);
        assert!(text.contains("\x1b["), "expected ANSI colors: {text:?}");
        assert_eq!(
            crate::style::strip_ansi(&text),
            "(0s) tool0, tool1, tool2, tool3"
        );
        crate::style::set_enabled(false);
    }

    #[test]
    fn render_is_plain_text_when_style_is_disabled() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let tools = tools_with(&[ToolState::Success]);
        assert_eq!(render(Duration::from_secs(0), &tools, None), "(0s) tool0");
    }

    #[test]
    fn render_shows_whole_seconds() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let tools = tools_with(&[ToolState::Pending]);
        assert!(render(Duration::from_secs(3), &tools, None).starts_with("(3s) "));
        // Sub-second fractions round down.
        // 不足一秒的小数部分向下取整。
        assert!(render(Duration::from_millis(3_900), &tools, None).starts_with("(3s) "));
    }

    #[test]
    fn render_truncates_to_max_width() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let tools = tools_with(&[ToolState::Pending, ToolState::Pending]);
        let text = render(Duration::from_secs(0), &tools, Some(8));
        assert_eq!(text.chars().count(), 8);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn render_does_not_truncate_without_a_usable_width() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let tools = tools_with(&[ToolState::Pending]);
        assert_eq!(
            render(Duration::from_secs(0), &tools, Some(0)),
            "(0s) tool0"
        );
        assert_eq!(render(Duration::from_secs(0), &tools, None), "(0s) tool0");
    }

    #[test]
    fn disabled_line_writes_nothing() {
        let capture = crate::console::capture();
        let mut line = ProgressLine::with_options(vec!["read".into()], false, None);
        line.running(0);
        line.succeed(0);
        line.fail(1);
        line.tick();
        line.suspend();
        line.finish();
        assert_eq!(capture.text(), "");
    }

    #[test]
    fn new_is_disabled_in_test_builds() {
        // `cfg!(test)` is true here, so the real terminal is never touched.
        // 此处 `cfg!(test)` 为真，真实终端绝不会被触碰。
        let line = ProgressLine::new(vec!["read".into()]);
        assert!(!line.enabled);
    }

    #[test]
    fn draw_to_writes_the_rendered_line() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let mut line = ProgressLine::with_options(vec!["read".into(), "edit".into()], true, None);
        line.tools[0].state = ToolState::Running;
        let mut out = Vec::new();
        line.draw_to(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("read"));
        assert!(text.contains("edit"));
    }

    #[test]
    fn draw_with_records_the_drawn_state_when_enabled() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let mut line = ProgressLine::with_options(vec!["read".into()], true, None);
        let mut out = Vec::new();
        line.draw_with(&mut out);
        assert!(line.drawn);
        assert!(!out.is_empty());
    }

    #[test]
    fn suspend_with_erases_a_drawn_line_when_enabled() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let mut line = ProgressLine::with_options(vec!["read".into()], true, None);
        line.drawn = true;
        let mut out = Vec::new();
        line.suspend_with(&mut out);
        assert!(!line.drawn);
        assert!(!out.is_empty());
    }

    #[test]
    fn finish_with_writes_a_terminating_line_when_enabled() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let mut line = ProgressLine::with_options(vec!["read".into()], true, None);
        line.drawn = true;
        let mut out = Vec::new();
        line.finish_with(&mut out);
        assert!(!line.drawn);
        assert!(out.ends_with(b"\n"));
    }

    #[test]
    fn finish_to_writes_a_newline_only_after_a_draw() {
        let _guard = crate::test_support::STYLE_LOCK.lock().unwrap();
        crate::style::set_enabled(false);
        let mut line = ProgressLine::with_options(vec!["read".into()], true, None);

        let mut out = Vec::new();
        line.finish_to(&mut out).unwrap();
        assert!(out.is_empty());

        line.drawn = true;
        let mut out = Vec::new();
        line.finish_to(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.ends_with('\n'));
        assert!(!line.drawn);
    }
}
