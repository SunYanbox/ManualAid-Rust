//! Terminal editor for the completion input: owns the crossterm event
//! loop and raw-mode lifecycle, while the pure state transitions live in
//! [`super::state`]. The key source and writer are injectable so the loop
//! is unit-testable without a TTY.
//! 补全输入的终端编辑器：负责 crossterm 事件循环与 raw mode 生命周期，
//! 纯状态转移位于 [`super::state`]。按键来源与 writer 可注入，使循环无需
//! TTY 即可单元测试。

use std::io::{self, IsTerminal, Write};
use std::sync::Arc;

use super::candidates::Candidate;
use super::state::{CompletionState, InputHistory, Key, Outcome, Trigger};

/// Maximum number of suggestion rows rendered at once.
/// 单次渲染的建议行数上限。
const MAX_SUGGESTIONS: usize = 8;

/// Maximum characters kept from a candidate description in the suggestion
/// panel; longer descriptions are truncated with `…` so one candidate stays
/// on one terminal row.
/// 建议面板中候选描述保留的最大字符数；更长的描述以 `…` 截断，保证
/// 每个候选只占一行终端。
const DESCRIPTION_MAX_CHARS: usize = 80;

/// Layout limits for the suggestion panel: the terminal width used to keep
/// one candidate on one row, and the number of rows that fit below the input
/// line before the terminal would scroll.
/// 建议面板的布局限制：终端宽度（让每个候选只占一行）与输入行下方可容纳
/// 的行数（超过会导致终端滚动）。
#[derive(Debug, Clone, Copy)]
struct PanelLayout {
    /// Terminal width in columns; `None` disables width-based truncation.
    /// 终端列数；`None` 表示不按宽度截断。
    max_width: Option<usize>,
}

impl PanelLayout {
    /// Query the terminal for its width. The panel height stays at
    /// [`MAX_SUGGESTIONS`] regardless of where the caret sits: when the
    /// terminal scrolls, the drawn rows and the caret move up together, so
    /// the relative `MoveUp` back to the input line stays correct and no
    /// caret-based limit is needed.
    /// 查询终端宽度。面板高度恒为 [`MAX_SUGGESTIONS`]，与光标位置无关：
    /// 终端滚动会把已绘制行与光标一起上移，回到输入行的相对 `MoveUp`
    /// 依然正确，因此无需按光标位置限制高度。
    fn detect() -> Self {
        // Outside an interactive terminal the console is shared with other
        // processes, so no reliable width can be read.
        // 非交互终端下控制台与其他进程共享，读不到可靠的宽度。
        let max_width = if is_interactive() {
            crossterm::terminal::size()
                .ok()
                .map(|(cols, _)| cols as usize)
        } else {
            None
        };
        Self { max_width }
    }

    /// Layout with an explicit width, for tests without a real terminal.
    /// 以显式宽度构建布局，供无真实终端的测试使用。
    #[cfg(test)]
    fn with_width(max_width: Option<usize>) -> Self {
        Self { max_width }
    }
}

/// One editor-level input event. Ctrl+D and Ctrl+C are editor concerns and
/// never reach the completion state machine, whose [`Key`] stays focused on
/// buffer editing and selection.
/// 编辑器级输入事件。Ctrl+D 与 Ctrl+C 是编辑器关注点，不进入补全状态机，
/// 使 [`Key`] 专注于缓冲编辑与选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorEvent {
    /// A regular key, forwarded to the completion state machine.
    /// 普通按键，转发给补全状态机。
    Key(Key),
    /// Ctrl+D on an empty line: end of input.
    /// 空行上的 Ctrl+D：输入结束。
    CtrlD,
    /// Ctrl+C: clear the whole buffer and start over.
    /// Ctrl+C：清空整个缓冲重新输入。
    CtrlC,
}

/// Read one line with completion; falls back to the plain scripted
/// `read_line` when this process is not interactive, keeping the existing
/// test-input mechanism untouched.
/// 读取一行并补全；进程非交互时回退到普通的脚本化 `read_line`，保持
/// 现有测试输入机制不变。
pub(crate) fn read_line_with_completion<F>(
    prompt: &str,
    history: Option<Arc<InputHistory>>,
    candidates_fn: F,
) -> Option<String>
where
    F: FnMut(&CompletionState) -> Vec<Candidate>,
{
    if !is_interactive() {
        crate::console::out_print!("{prompt}");
        crate::console::flush();
        let line = crate::commands::loop_cli::utils::read_line()?;
        if let Some(history) = &history {
            history.push(line.trim());
        }
        return Some(line);
    }
    let _raw = crate::pager::RawModeGuard::enable().ok()?;
    run_completion_loop(io::stdout(), prompt, history, candidates_fn, read_key_event)
}

/// Whether completion editing is allowed in this process: test builds never
/// enter raw mode, captured integration tests route through the scripted
/// input queue, and a non-TTY stdin/stdout cannot render the suggestion
/// panel.
/// 当前进程是否允许补全编辑：测试构建永不进入 raw mode；捕获状态的集成
/// 测试走脚本输入队列；stdin/stdout 非 TTY 无法渲染建议面板。
fn is_interactive() -> bool {
    !cfg!(test)
        && !crate::console::is_capturing()
        && io::stdin().is_terminal()
        && io::stdout().is_terminal()
}

/// Read one crossterm event and convert it into an [`EditorEvent`];
/// non-key events and unsupported keys are skipped.
/// 读取一个 crossterm 事件并转换为 [`EditorEvent`]；非按键事件与不支持的
/// 按键被跳过。
fn read_key_event() -> io::Result<EditorEvent> {
    use crossterm::event::{self, Event};
    loop {
        if let Event::Key(key) = event::read()?
            && let Some(event) = convert_crossterm_key(key)
        {
            return Ok(event);
        }
    }
}

/// Convert a crossterm key event into an editor event, ignoring key-release
/// reports and unsupported modifier combinations.
/// 将 crossterm 按键事件转换为编辑器事件，忽略按键释放报告与不支持的
/// 修饰键组合。
fn convert_crossterm_key(key: crossterm::event::KeyEvent) -> Option<EditorEvent> {
    use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

    if key.kind == KeyEventKind::Release {
        return None;
    }
    match key.code {
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(EditorEvent::CtrlD)
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(EditorEvent::CtrlC)
        }
        // Ctrl+Left/Right move by whole words; terminals report either the
        // arrow with CONTROL or the Emacs bindings C-b/C-f, so accept both.
        // Ctrl+Left/Right 按词移动；终端可能上报带 CONTROL 的方向键或
        // Emacs 绑定 C-b/C-f，两者都接受。
        KeyCode::Left => Some(EditorEvent::Key(
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Key::CtrlLeft
            } else {
                Key::Left
            },
        )),
        KeyCode::Right => Some(EditorEvent::Key(
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                Key::CtrlRight
            } else {
                Key::Right
            },
        )),
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(EditorEvent::Key(Key::CtrlLeft))
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(EditorEvent::Key(Key::CtrlRight))
        }
        KeyCode::Char(ch) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            Some(EditorEvent::Key(Key::Char(ch)))
        }
        KeyCode::Backspace => Some(EditorEvent::Key(Key::Backspace)),
        KeyCode::Up => Some(EditorEvent::Key(Key::Up)),
        KeyCode::Down => Some(EditorEvent::Key(Key::Down)),
        KeyCode::Tab => Some(EditorEvent::Key(Key::Tab)),
        KeyCode::Esc => Some(EditorEvent::Key(Key::Esc)),
        KeyCode::Enter => Some(EditorEvent::Key(Key::Enter)),
        _ => None,
    }
}

/// Drive the completion loop against an injectable writer and key source.
/// Returns the submitted line, or `None` on EOF (Ctrl+D on an empty buffer).
/// 以可注入的 writer 与按键来源驱动补全循环。返回提交的行；EOF（空缓冲
/// 上的 Ctrl+D）返回 `None`。
fn run_completion_loop<W, F, K>(
    mut writer: W,
    prompt: &str,
    history: Option<Arc<InputHistory>>,
    mut candidates_fn: F,
    mut read_key: K,
) -> Option<String>
where
    W: Write,
    F: FnMut(&CompletionState) -> Vec<Candidate>,
    K: FnMut() -> io::Result<EditorEvent>,
{
    let prompt_width = display_width(prompt);
    let mut state = match &history {
        Some(history) => CompletionState::with_history(history.clone()),
        None => CompletionState::new(),
    };
    refresh_candidates(&mut state, &mut candidates_fn);
    let mut prev_suggestion_lines = render(&mut writer, prompt, &state, 0, PanelLayout::detect())
        .expect("initial render must succeed");

    loop {
        let event = read_key().ok()?;
        match event {
            EditorEvent::Key(key) => {
                // Any buffer or cursor edit can change the active completion
                // token; selection moves, history recalls and Esc keep the
                // current candidate set.
                // 一切缓冲或光标编辑都可能改变活动补全 token；选择移动、
                // 历史切换与 Esc 保持当前候选集不变。
                let refresh = matches!(
                    key,
                    Key::Char(_)
                        | Key::Backspace
                        | Key::Tab
                        | Key::Left
                        | Key::Right
                        | Key::CtrlLeft
                        | Key::CtrlRight
                );
                match state.handle_key(key) {
                    Outcome::Handled => {
                        if refresh {
                            refresh_candidates(&mut state, &mut candidates_fn);
                        }
                        prev_suggestion_lines = render(
                            &mut writer,
                            prompt,
                            &state,
                            prev_suggestion_lines,
                            PanelLayout::detect(),
                        )
                        .ok()?;
                    }
                    Outcome::Submit(line) => {
                        clear_suggestions_and_submit(
                            &mut writer,
                            prompt_width + display_width(&line),
                            prev_suggestion_lines,
                        )
                        .ok()?;
                        if let Some(history) = &history {
                            history.push(line.trim());
                        }
                        return Some(line);
                    }
                }
            }
            EditorEvent::CtrlD => {
                if state.buffer().is_empty() {
                    return None;
                }
                // Ctrl+D on a non-empty buffer has no defined action here.
                // 非空缓冲上的 Ctrl+D 在此没有定义动作。
            }
            EditorEvent::CtrlC => {
                // Ctrl+C on an empty buffer means "abort the input", which
                // lets the loop exit; on a non-empty buffer it clears the
                // typed text and keeps the editor open.
                // 空缓冲上的 Ctrl+C 表示“中止输入”，让 loop 退出；非空
                // 缓冲则清空已输入内容并保持编辑器打开。
                if state.buffer().is_empty() {
                    return None;
                }
                state = match &history {
                    Some(history) => CompletionState::with_history(history.clone()),
                    None => CompletionState::new(),
                };
                refresh_candidates(&mut state, &mut candidates_fn);
                prev_suggestion_lines = render(
                    &mut writer,
                    prompt,
                    &state,
                    prev_suggestion_lines,
                    PanelLayout::detect(),
                )
                .ok()?;
            }
        }
    }
}

/// Query the candidate provider for the current trigger and install the
/// result; when no token is active the suggestion panel stays closed.
/// 针对当前触发词查询候选提供者并安装结果；无活动 token 时建议面板保持
/// 关闭。
fn refresh_candidates<F>(state: &mut CompletionState, candidates_fn: &mut F)
where
    F: FnMut(&CompletionState) -> Vec<Candidate>,
{
    let candidates = if state.active_query().is_some() {
        candidates_fn(state)
    } else {
        Vec::new()
    };
    state.set_candidates(candidates);
}

/// Redraw the input line and suggestion rows; returns how many rows the
/// suggestion area now occupies so the next render can clear exactly those
/// rows.
/// 重绘输入行与建议行；返回建议区当前占用的行数，供下一次渲染精确清除。
fn render<W: Write>(
    writer: &mut W,
    prompt: &str,
    state: &CompletionState,
    prev_suggestion_lines: usize,
    layout: PanelLayout,
) -> io::Result<usize> {
    use crossterm::cursor::{MoveToColumn, MoveUp};
    use crossterm::terminal::{Clear, ClearType};

    // Clear the input line first so the new prompt and buffer overwrite the
    // previous content exactly.
    // 先清除输入行，让新的提示符与缓冲精确覆盖旧内容。
    crossterm::execute!(writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    write!(writer, "{prompt}{}", state.buffer())?;

    let candidates = state.candidates();
    let new_lines = if candidates.is_empty() {
        0
    } else {
        candidates.len().min(MAX_SUGGESTIONS)
    };
    // Keep the highlighted candidate visible by sliding the window when the
    // selection reaches the bottom edge.
    // 当高亮移到候选列表底部时滑动窗口，使其始终可见。
    let start = if candidates.len() > MAX_SUGGESTIONS {
        state
            .selected()
            .saturating_sub(MAX_SUGGESTIONS - 1)
            .min(candidates.len() - new_lines)
    } else {
        0
    };
    let selected_offset = state.selected().saturating_sub(start);
    // Clear old rows and write new suggestions in one pass; using the
    // maximum of the two sizes keeps the visible area stable without
    // leaving stale rows behind.
    // 一轮完成旧行清空与新建议写入；取两者较大值保持可见区域稳定，
    // 不残留过期行。
    let total_lines = prev_suggestion_lines.max(new_lines);
    let path_trigger = matches!(state.active_trigger(), Some(Trigger::Path));
    for (index, candidate) in candidates.iter().skip(start).take(new_lines).enumerate() {
        write!(writer, "\r\n")?;
        crossterm::execute!(writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
        let marker = if index == selected_offset { '>' } else { ' ' };
        write!(
            writer,
            "{}",
            render_suggestion_row(candidate, marker, path_trigger, layout.max_width)
        )?;
    }
    // Clear any rows the previous render occupied but the current candidate
    // list no longer fills.
    // 清除上一轮占用但当前候选列表不再填满的行。
    for _ in new_lines..total_lines {
        write!(writer, "\r\n")?;
        crossterm::execute!(writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    }

    // Move the cursor back onto the input line at its logical position;
    // the visible column is the prompt width plus the display width of the
    // text before the cursor, so CJK characters keep the caret aligned.
    // 将光标移回输入行的逻辑位置；可见列等于提示符宽度加光标前文本的显示
    // 宽度，使 CJK 字符下光标依然对齐。
    if total_lines > 0 {
        crossterm::execute!(writer, MoveUp(total_lines as u16))?;
    }
    let caret_column = display_width(prompt) + display_width(&state.buffer()[..state.cursor()]);
    crossterm::execute!(writer, MoveToColumn(caret_column as u16))?;
    writer.flush()?;
    Ok(total_lines)
}

/// Clear the suggestion rows after a submit and move the cursor to a fresh
/// line so following output never overlaps the editor.
/// 提交后清除建议行并把光标移到新行，使后续输出不与编辑器重叠。
fn clear_suggestions_and_submit<W: Write>(
    writer: &mut W,
    input_end_column: usize,
    prev_suggestion_lines: usize,
) -> io::Result<()> {
    use crossterm::cursor::{MoveToColumn, MoveUp};
    use crossterm::terminal::{Clear, ClearType};

    for _ in 0..prev_suggestion_lines {
        write!(writer, "\r\n")?;
        crossterm::execute!(writer, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    }
    if prev_suggestion_lines > 0 {
        crossterm::execute!(writer, MoveUp(prev_suggestion_lines as u16))?;
    }
    crossterm::execute!(writer, MoveToColumn(input_end_column as u16))?;
    write!(writer, "\r\n")?;
    writer.flush()
}

/// Truncate a description to [`DESCRIPTION_MAX_CHARS`] characters, appending
/// `…` when truncated so the user can tell more text follows.
/// 将描述截断为 [`DESCRIPTION_MAX_CHARS`] 个字符；截断时追加 `…`，让用户
/// 知道还有更多内容。
fn truncate_description(description: &str) -> String {
    if description.chars().count() <= DESCRIPTION_MAX_CHARS {
        description.to_owned()
    } else {
        let mut truncated: String = description.chars().take(DESCRIPTION_MAX_CHARS).collect();
        truncated.push('…');
        truncated
    }
}

/// Visible column width after stripping ANSI style sequences: CJK and
/// full-width characters occupy two terminal columns, so the cursor lands
/// correctly after a localized prompt.
/// 去除 ANSI 样式序列后的可见列宽：CJK 与全角字符占两列终端宽度，
/// 本地化提示后光标才能落在正确位置。
fn display_width(text: &str) -> usize {
    crate::style::strip_ansi(text).chars().map(char_width).sum()
}

/// Terminal columns occupied by one character: two for CJK and full-width
/// forms, one otherwise.
/// 单个字符占用的终端列数：CJK 与全角形式为两列，其余为一列。
fn char_width(ch: char) -> usize {
    let code = ch as u32;
    if (0x1100..=0x115F).contains(&code)
        || (0x2E80..=0xA4CF).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0xFE30..=0xFE4F).contains(&code)
        || (0xFF00..=0xFF60).contains(&code)
        || (0xFFE0..=0xFFE6).contains(&code)
    {
        2
    } else {
        1
    }
}

/// Render one suggestion row within `max_width` display columns so the row
/// never wraps onto a second terminal row; without a known width the
/// description is capped at [`DESCRIPTION_MAX_CHARS`] characters as before.
/// 在 `max_width` 个显示列内渲染一条建议行，使其不会折到第二行；宽度未知
/// 时仍按 [`DESCRIPTION_MAX_CHARS`] 个字符限制描述。
fn render_suggestion_row(
    candidate: &Candidate,
    marker: char,
    path_trigger: bool,
    max_width: Option<usize>,
) -> String {
    let label = if path_trigger {
        format!("@{}", candidate.label)
    } else {
        candidate.label.clone()
    };
    let label = if candidate.dimmed {
        crate::style::muted(&label)
    } else {
        label
    };
    let head = format!("{marker} {label}");
    let Some(width) = max_width else {
        return if candidate.description.is_empty() {
            head
        } else {
            format!("{head}  {}", truncate_description(&candidate.description))
        };
    };
    if candidate.description.is_empty() {
        return truncate_to_width(&head, width);
    }
    // Two spaces separate the label from its description.
    // 标签与描述之间以两个空格分隔。
    const SEPARATOR_WIDTH: usize = 2;
    let head_width = display_width(&head);
    if head_width + SEPARATOR_WIDTH >= width {
        return truncate_to_width(&head, width);
    }
    let description =
        truncate_to_width(&candidate.description, width - head_width - SEPARATOR_WIDTH);
    format!("{head}  {description}")
}

/// Truncate `text` to `max_width` display columns, appending `…` when it was
/// cut. Styling is dropped on truncation: slicing inside an ANSI sequence
/// would corrupt the stream, mirroring the progress line's approach.
/// 将 `text` 截断到 `max_width` 个显示列，截断时追加 `…`。截断会丢弃样式：
/// 在 ANSI 序列中间截断会破坏输出流，与进度行的做法一致。
fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if display_width(text) <= max_width {
        return text.to_owned();
    }
    let budget = max_width - 1;
    let mut kept = String::new();
    let mut used = 0;
    for ch in crate::style::strip_ansi(text).chars() {
        let width = char_width(ch);
        if used + width > budget {
            break;
        }
        kept.push(ch);
        used += width;
    }
    kept.push('…');
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::loop_cli::utils::push_test_input;

    /// Build a fixed candidate provider: `/help` and `/history`, filtered by
    /// the active query just like the real providers would.
    /// 构造固定候选提供者：`/help` 与 `/history`，按活动查询过滤，与真实
    /// 提供者行为一致。
    fn command_candidates(state: &CompletionState) -> Vec<Candidate> {
        let query = state.active_query().unwrap_or("");
        if query.is_empty() {
            return Vec::new();
        }
        vec![
            Candidate {
                label: "/help".to_owned(),
                description: "Show help".to_owned(),
                is_dir: false,
                dimmed: false,
            },
            Candidate {
                label: "/history".to_owned(),
                description: "Show history".to_owned(),
                is_dir: false,
                dimmed: false,
            },
        ]
        .into_iter()
        .filter(|candidate| {
            candidate
                .label
                .to_lowercase()
                .starts_with(&query.to_lowercase())
        })
        .collect()
    }

    #[test]
    fn command_candidates_without_a_query_is_empty() {
        // No active token means the provider returns nothing, keeping the
        // suggestion panel closed.
        // 无活动 token 时提供者返回空，建议面板保持关闭。
        assert!(command_candidates(&CompletionState::new()).is_empty());
    }

    /// Run the completion loop over a fixed event sequence.
    /// 以固定事件序列运行补全循环。
    fn run_with_events(
        writer: &mut Vec<u8>,
        prompt: &str,
        candidates_fn: impl FnMut(&CompletionState) -> Vec<Candidate>,
        events: &[EditorEvent],
    ) -> Option<String> {
        let mut iter = events.iter();
        run_completion_loop(writer, prompt, None, candidates_fn, || {
            iter.next()
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "no more events"))
        })
    }

    #[test]
    fn convert_key_maps_ctrl_d_ctrl_c_and_regular_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            Some(EditorEvent::CtrlD)
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(EditorEvent::CtrlC)
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Char('x')))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Backspace))
        ));
    }

    #[test]
    fn convert_key_maps_special_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Backspace))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Up))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Down))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Tab))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Esc))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Enter))
        ));
    }

    #[test]
    fn convert_key_ignores_release_events() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
        let key = KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: crossterm::event::KeyEventState::NONE,
        };
        assert_eq!(convert_crossterm_key(key), None);
    }

    #[test]
    fn convert_key_ignores_unsupported_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        assert_eq!(
            convert_crossterm_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            None
        );
    }

    #[test]
    fn convert_key_maps_cursor_movement_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Left))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            Some(EditorEvent::Key(Key::Right))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL)),
            Some(EditorEvent::Key(Key::CtrlLeft))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL)),
            Some(EditorEvent::Key(Key::CtrlRight))
        ));
        // Emacs-style word movement bindings are accepted as aliases.
        // Emacs 风格的按词移动绑定作为别名接受。
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            Some(EditorEvent::Key(Key::CtrlLeft))
        ));
        assert!(matches!(
            convert_crossterm_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)),
            Some(EditorEvent::Key(Key::CtrlRight))
        ));
    }

    #[test]
    fn enter_submits_the_buffer() {
        let mut out = Vec::new();
        let events = [
            EditorEvent::Key(Key::Char('/')),
            EditorEvent::Key(Key::Char('h')),
            EditorEvent::Key(Key::Enter),
        ];
        assert_eq!(
            run_with_events(&mut out, "> ", |_| Vec::new(), &events),
            Some("/h".to_owned())
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("> /h"));
    }

    #[test]
    fn ctrl_d_on_an_empty_buffer_returns_none() {
        let mut out = Vec::new();
        let events = [EditorEvent::CtrlD];
        assert_eq!(
            run_with_events(&mut out, "> ", |_| Vec::new(), &events),
            None
        );
    }

    #[test]
    fn ctrl_d_on_a_non_empty_buffer_keeps_editing() {
        let mut out = Vec::new();
        let events = [
            EditorEvent::Key(Key::Char('a')),
            EditorEvent::CtrlD,
            EditorEvent::Key(Key::Enter),
        ];
        assert_eq!(
            run_with_events(&mut out, "> ", |_| Vec::new(), &events),
            Some("a".to_owned())
        );
    }

    #[test]
    fn ctrl_c_clears_the_buffer() {
        let mut out = Vec::new();
        let events = [
            EditorEvent::Key(Key::Char('a')),
            EditorEvent::Key(Key::Char('b')),
            EditorEvent::CtrlC,
            EditorEvent::Key(Key::Enter),
        ];
        assert_eq!(
            run_with_events(&mut out, "> ", |_| Vec::new(), &events),
            Some(String::new())
        );
    }

    #[test]
    fn ctrl_c_on_an_empty_buffer_returns_none() {
        let mut out = Vec::new();
        let events = [EditorEvent::CtrlC];
        assert_eq!(
            run_with_events(&mut out, "> ", |_| Vec::new(), &events),
            None
        );
    }

    #[test]
    fn suggestions_are_rendered_before_enter() {
        let mut out = Vec::new();
        let events = [
            EditorEvent::Key(Key::Char('/')),
            EditorEvent::Key(Key::Char('h')),
            EditorEvent::Key(Key::Enter),
        ];
        assert_eq!(
            run_with_events(&mut out, "> ", command_candidates, &events),
            Some("/help".to_owned())
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("/help"));
        assert!(text.contains("/history"));
    }

    #[test]
    fn display_width_counts_cjk_as_two_columns() {
        assert_eq!(display_width("ab"), 2);
        assert_eq!(display_width("中文"), 4);
        assert_eq!(display_width("a中"), 3);
    }

    #[test]
    fn render_slides_window_when_selection_reaches_bottom() {
        let mut out = Vec::new();
        let many: Vec<Candidate> = (0..10)
            .map(|index| Candidate {
                label: format!("/item{index}"),
                description: String::new(),
                is_dir: false,
                dimmed: false,
            })
            .collect();
        let mut state = CompletionState::new();
        state.set_candidates(many);
        // Move the selection to the last candidate so the window must slide.
        for _ in 0..9 {
            let _ = state.handle_key(Key::Down);
        }
        let lines = render(&mut out, "> ", &state, 0, PanelLayout::with_width(None)).unwrap();
        assert!(lines > 0);
        let text = String::from_utf8(out).unwrap();
        // The last candidate must be among the rendered rows.
        assert!(text.contains("/item9"));
    }

    #[test]
    fn render_marks_dimmed_path_candidate_as_muted() {
        let mut out = Vec::new();
        let mut state = CompletionState::new();
        for ch in "@sub".chars() {
            let _ = state.handle_key(Key::Char(ch));
        }
        let candidates = vec![Candidate {
            label: "sub".to_owned(),
            description: String::new(),
            is_dir: true,
            dimmed: true,
        }];
        state.set_candidates(candidates);
        let lines = render(&mut out, "> ", &state, 0, PanelLayout::with_width(None)).unwrap();
        assert_eq!(lines, 1);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("@sub"));
        assert!(text.contains("\x1b["));
    }

    #[test]
    fn truncate_description_keeps_short_text_and_truncates_long_text() {
        assert_eq!(truncate_description("short"), "short");
        let long = "a".repeat(90);
        let truncated = truncate_description(&long);
        assert_eq!(truncated.chars().count(), 81);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn truncate_to_width_handles_zero_exact_and_cjk_widths() {
        assert_eq!(truncate_to_width("abc", 0), "");
        assert_eq!(truncate_to_width("abc", 3), "abc");
        assert_eq!(truncate_to_width("abcd", 3), "ab…");
        // CJK characters occupy two columns, so only one fits in three.
        // CJK 字符占两列，三列宽度下只放得下一个。
        assert_eq!(truncate_to_width("中文", 3), "中…");
    }

    #[test]
    fn suggestion_row_stays_within_the_terminal_width() {
        let candidate = Candidate {
            label: "project-.agents-job-description-analyzer".to_owned(),
            description: "Analyze job postings, calculate match scores, identify gaps, and create application strategy".to_owned(),
            is_dir: false,
            dimmed: false,
        };
        let row = render_suggestion_row(&candidate, '>', false, Some(60));
        // A row wider than the terminal would wrap and break the row-count
        // bookkeeping that the redraw logic relies on.
        // 超过终端宽度的行会折行，破坏重绘逻辑依赖的行数记账。
        assert!(
            display_width(&row) <= 60,
            "row exceeded the width budget: {row:?}"
        );
        assert!(row.ends_with('…'));
    }

    #[test]
    fn suggestion_row_budget_counts_cjk_descriptions_as_two_columns() {
        let candidate = Candidate {
            label: "/help".to_owned(),
            description: "显示帮助与可用命令说明的中文描述文本".to_owned(),
            is_dir: false,
            dimmed: false,
        };
        let row = render_suggestion_row(&candidate, ' ', false, Some(30));
        assert!(display_width(&row) <= 30, "row was {row:?}");
    }

    #[test]
    fn suggestion_row_without_a_width_keeps_the_character_cap() {
        let candidate = Candidate {
            label: "/help".to_owned(),
            description: "a".repeat(90),
            is_dir: false,
            dimmed: false,
        };
        let row = render_suggestion_row(&candidate, ' ', false, None);
        assert!(row.ends_with('…'));
        assert_eq!(row.matches('a').count(), 80);
    }

    #[test]
    fn suggestion_row_drops_the_description_when_the_label_fills_the_budget() {
        let candidate = Candidate {
            label: "project-.agents-demo".to_owned(),
            description: "Demo skill".to_owned(),
            is_dir: false,
            dimmed: false,
        };
        // The marker and the label alone exceed the budget, so the
        // description is dropped and the label itself is truncated.
        // 标记与标签本身已超出预算，因此丢弃描述并截断标签本身。
        let row = render_suggestion_row(&candidate, '>', false, Some(10));
        assert!(display_width(&row) <= 10, "row was {row:?}");
        assert!(!row.contains("Demo skill"));
        assert!(row.ends_with('…'));
    }

    #[test]
    fn render_shows_every_candidate_up_to_the_panel_limit() {
        let mut out = Vec::new();
        let many: Vec<Candidate> = (0..3)
            .map(|index| Candidate {
                label: format!("/item{index}"),
                description: String::new(),
                is_dir: false,
                dimmed: false,
            })
            .collect();
        let mut state = CompletionState::new();
        state.set_candidates(many);
        // The panel height must not depend on where the caret happens to sit,
        // so every candidate is drawn even when output has scrolled the input
        // line to the bottom of the terminal.
        // 面板高度不得取决于光标位置：即使输出已把输入行挤到终端底部，
        // 每个候选也应照常绘制。
        let lines = render(&mut out, "> ", &state, 0, PanelLayout::with_width(Some(80))).unwrap();
        assert_eq!(lines, 3);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("/item0"));
        assert!(text.contains("/item1"));
        assert!(text.contains("/item2"));
    }

    #[test]
    fn tab_fills_the_selected_suggestion() {
        let mut out = Vec::new();
        let events = [
            EditorEvent::Key(Key::Char('/')),
            EditorEvent::Key(Key::Char('h')),
            EditorEvent::Key(Key::Tab),
            EditorEvent::Key(Key::Enter),
        ];
        assert_eq!(
            run_with_events(&mut out, "> ", command_candidates, &events),
            Some("/help".to_owned())
        );
    }

    #[test]
    fn read_line_with_completion_falls_back_to_scripted_input_in_tests() {
        let capture = crate::console::capture();
        push_test_input(&["hello"]);
        assert_eq!(
            read_line_with_completion("> ", None, |_| Vec::new()).as_deref(),
            Some("hello")
        );
        assert_eq!(capture.text(), "> ");
    }

    #[test]
    fn submitted_lines_are_recorded_into_the_shared_history() {
        let history = Arc::new(InputHistory::new());
        push_test_input(&["hello"]);
        assert_eq!(
            read_line_with_completion("> ", Some(history.clone()), |_| Vec::new()).as_deref(),
            Some("hello")
        );
        // The recalled entry proves the submission reached the history.
        // 能调出该条目证明提交已写入历史。
        let mut state = CompletionState::with_history(history);
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "hello");
    }
}
