//! Pure state machine for the interactive completion input: key events
//! mutate the buffer and suggestion selection without touching the
//! terminal, so every transition is unit-testable without a TTY.
//! 交互式补全输入的纯状态机：按键事件只改变缓冲与建议选中状态，不接触
//! 终端，因此每个状态转移都能在无 TTY 环境下单元测试。

use std::sync::{Arc, Mutex};

use super::candidates::Candidate;

/// What kind of token currently drives completion.
/// 当前驱动补全的 token 类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// A leading `/` token for built-in commands and skills.
    /// 行首 `/` token，用于内置命令与技能。
    Command,
    /// A leading or space-separated `@` token for project paths.
    /// 行首或空格分隔的 `@` token，用于项目路径。
    Path,
}

/// A key press reduced to the events the completion editor cares about.
/// 按键归约为补全编辑器关心的事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    /// A printable character to insert at the cursor.
    /// 在光标处插入的可打印字符。
    Char(char),
    Backspace,
    /// Move the cursor one character left.
    /// 光标左移一个字符。
    Left,
    /// Move the cursor one character right.
    /// 光标右移一个字符。
    Right,
    /// Move the cursor one word left (Ctrl+Left).
    /// 光标左移一个词（Ctrl+Left）。
    CtrlLeft,
    /// Move the cursor one word right (Ctrl+Right).
    /// 光标右移一个词（Ctrl+Right）。
    CtrlRight,
    /// Select the previous suggestion, or recall an older history entry.
    /// 选择上一个建议，或调出更早的历史输入。
    Up,
    /// Select the next suggestion, or recall a newer history entry.
    /// 选择下一个建议，或调出更新的历史输入。
    Down,
    Tab,
    Esc,
    Enter,
}

/// Shared in-memory input history for the interactive loop: entries are
/// kept oldest-to-newest and a newly submitted line replaces any earlier
/// duplicate, so the same input keeps only its latest occurrence.
/// 交互式 loop 的共享内存输入历史：条目按从旧到新保存，新提交的行会替换
/// 更早的相同条目，使相同输入只保留最新一条。
#[derive(Debug, Default)]
pub(crate) struct InputHistory {
    entries: Mutex<Vec<String>>,
}

impl InputHistory {
    /// Create an empty history.
    /// 创建空历史。
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Record one submitted line: earlier duplicates of the same text are
    /// dropped so only the newest occurrence stays, and blank lines are
    /// ignored.
    /// 记录一条已提交输入：先删除更早的相同条目再追加，使相同输入只保留
    /// 最新一条；空白行被忽略。
    pub(crate) fn push(&self, line: &str) {
        if line.trim().is_empty() {
            return;
        }
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|entry| entry != line);
        entries.push(line.to_owned());
    }

    /// Copy of the entries ordered from oldest to newest.
    /// 返回从旧到新排序的条目副本。
    fn snapshot(&self) -> Vec<String> {
        self.entries.lock().unwrap().clone()
    }
}

/// Result of handling one key.
/// 处理一次按键的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The key changed internal state; the editor should redraw.
    /// 按键改变了内部状态；编辑器应重绘。
    Handled,
    /// Enter was pressed; the buffer content is the final input.
    /// 按下了 Enter；缓冲内容即最终输入。
    Submit(String),
}

/// Completion state kept between key presses.
/// 按键之间保持的补全状态。
#[derive(Debug)]
pub(crate) struct CompletionState {
    buffer: String,
    candidates: Vec<Candidate>,
    selected: usize,
    /// Byte index into `buffer`, always on a char boundary; Left/Right and
    /// Ctrl+Left/Right move it for mid-line editing.
    /// `buffer` 的字节索引，始终位于字符边界；Left/Right 与 Ctrl+Left/Right
    /// 移动它以支持行内编辑。
    cursor: usize,
    /// Session input history shared across editor rounds.
    /// 跨编辑轮次共享的会话输入历史。
    history: Option<Arc<InputHistory>>,
    /// History browsing position: index into the entries, or `None` while
    /// the draft buffer is shown.
    /// 历史浏览位置：条目索引；`None` 表示当前显示草稿缓冲。
    history_pos: Option<usize>,
    /// Buffer content saved when history browsing started, restored when
    /// browsing moves past the newest entry.
    /// 开始浏览历史时保存的缓冲内容，浏览越过最新条目时恢复。
    draft: String,
}

impl CompletionState {
    /// Create an empty state with no suggestions and no history.
    /// 创建无建议、无历史的空状态。
    pub(crate) fn new() -> Self {
        Self {
            buffer: String::new(),
            candidates: Vec::new(),
            selected: 0,
            cursor: 0,
            history: None,
            history_pos: None,
            draft: String::new(),
        }
    }

    /// Create a state that navigates the shared session history.
    /// 创建可浏览共享会话历史的状态。
    pub(crate) fn with_history(history: Arc<InputHistory>) -> Self {
        Self {
            history: Some(history),
            ..Self::new()
        }
    }

    /// Current input line.
    /// 当前输入行。
    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Cursor position as a byte index into the buffer.
    /// 光标位置，为缓冲内的字节索引。
    pub(crate) fn cursor(&self) -> usize {
        self.cursor
    }

    /// Suggestions currently shown to the user.
    /// 当前展示给用户的建议。
    pub(crate) fn candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// Index of the highlighted suggestion; meaningful only when
    /// [`candidates`](Self::candidates) is non-empty.
    /// 高亮建议的索引；仅当 [`candidates`](Self::candidates) 非空时有意义。
    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    /// The token kind that should drive completion right now.
    /// 当前应驱动补全的 token 类型。
    pub(crate) fn active_trigger(&self) -> Option<Trigger> {
        self.active_token_range().map(|(trigger, _, _)| trigger)
    }

    /// Query text suitable for the candidate filters: commands keep the
    /// leading `/`, path queries drop the leading `@`.
    /// 适合候选过滤器的查询文本：命令保留起始 `/`，路径查询去掉 `@`。
    pub(crate) fn active_query(&self) -> Option<&str> {
        let (trigger, start, end) = self.active_token_range()?;
        let text = &self.buffer[start..end];
        Some(match trigger {
            Trigger::Command => text,
            Trigger::Path => &text[1..],
        })
    }

    /// Replace the suggestions; an empty list closes the suggestion panel
    /// and resets the highlight, a non-empty list resets a stale highlight.
    /// 替换建议；空列表关闭建议面板并重置高亮，非空列表修正过期高亮。
    pub(crate) fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        if self.candidates.is_empty() || self.selected >= self.candidates.len() {
            self.selected = 0;
        }
    }

    /// Apply one key press and report whether the input is complete.
    /// 应用一次按键并报告输入是否完成。
    pub(crate) fn handle_key(&mut self, key: Key) -> Outcome {
        match key {
            Key::Char(ch) => {
                self.buffer.insert(self.cursor, ch);
                self.cursor += ch.len_utf8();
                Outcome::Handled
            }
            Key::Backspace => {
                if let Some(previous) = self.previous_boundary() {
                    self.buffer.replace_range(previous..self.cursor, "");
                    self.cursor = previous;
                }
                Outcome::Handled
            }
            Key::Left => {
                if let Some(previous) = self.previous_boundary() {
                    self.cursor = previous;
                }
                Outcome::Handled
            }
            Key::Right => {
                if let Some(next) = self.next_boundary() {
                    self.cursor = next;
                }
                Outcome::Handled
            }
            Key::CtrlLeft => {
                self.cursor = self.word_boundary_left();
                Outcome::Handled
            }
            Key::CtrlRight => {
                self.cursor = self.word_boundary_right();
                Outcome::Handled
            }
            Key::Up => {
                if self.candidates.is_empty() {
                    // With no suggestion panel the arrows walk the session
                    // input history instead of the selection.
                    // 无建议面板时上下键切换会话输入历史而非选择。
                    self.recall_previous();
                } else {
                    self.selected = if self.selected == 0 {
                        self.candidates.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
                Outcome::Handled
            }
            Key::Down => {
                if self.candidates.is_empty() {
                    self.recall_next();
                } else {
                    self.selected = (self.selected + 1) % self.candidates.len();
                }
                Outcome::Handled
            }
            Key::Tab => {
                self.apply_selected();
                Outcome::Handled
            }
            Key::Esc => {
                // Close the suggestion panel without touching the buffer.
                // 关闭建议面板，不改动缓冲。
                self.candidates.clear();
                self.selected = 0;
                Outcome::Handled
            }
            Key::Enter => {
                // Enter completes like Tab when a suggestion is selected,
                // then submits the resulting buffer immediately.
                // Enter 在有选中建议时与 Tab 一样先补全，然后立即提交
                // 补全后的缓冲。
                self.apply_selected();
                Outcome::Submit(self.buffer.clone())
            }
        }
    }

    /// Byte index of the char boundary before the cursor, or `None` at the
    /// start of the buffer.
    /// 光标前最近字符边界的字节索引；已在缓冲开头时返回 `None`。
    fn previous_boundary(&self) -> Option<usize> {
        self.buffer[..self.cursor]
            .chars()
            .next_back()
            .map(|ch| self.cursor - ch.len_utf8())
    }

    /// Byte index of the char boundary after the cursor, or `None` at the
    /// end of the buffer.
    /// 光标后最近字符边界的字节索引；已在缓冲末尾时返回 `None`。
    fn next_boundary(&self) -> Option<usize> {
        self.buffer[self.cursor..]
            .chars()
            .next()
            .map(|ch| self.cursor + ch.len_utf8())
    }

    /// Move the cursor to the start of the current or previous word: skip
    /// whitespace, then the word characters (Ctrl+Left).
    /// 光标移到当前或上一个词的词首：先跳过空白，再跳过词字符（Ctrl+Left）。
    fn word_boundary_left(&self) -> usize {
        let head: Vec<char> = self.buffer[..self.cursor].chars().collect();
        let mut index = head.len();
        while index > 0 && head[index - 1].is_whitespace() {
            index -= 1;
        }
        while index > 0 && !head[index - 1].is_whitespace() {
            index -= 1;
        }
        head[..index]
            .iter()
            .map(|ch: &char| ch.len_utf8())
            .sum::<usize>()
    }

    /// Move the cursor to the start of the next word: skip the rest of the
    /// current word, then the whitespace run (Ctrl+Right).
    /// 光标移到下一个词的词首：先跳过当前词的剩余部分，再跳过空白串
    /// （Ctrl+Right）。
    fn word_boundary_right(&self) -> usize {
        let tail: Vec<char> = self.buffer[self.cursor..].chars().collect();
        let mut index = 0;
        while index < tail.len() && !tail[index].is_whitespace() {
            index += 1;
        }
        while index < tail.len() && tail[index].is_whitespace() {
            index += 1;
        }
        self.cursor
            + tail[..index]
                .iter()
                .map(|ch: &char| ch.len_utf8())
                .sum::<usize>()
    }

    /// Recall the previous (older) history entry; the current buffer is
    /// saved as a draft the first time browsing starts.
    /// 调出上一条（更早的）历史输入；首次开始浏览时把当前缓冲保存为草稿。
    fn recall_previous(&mut self) {
        let Some(entries) = self.history_entries() else {
            return;
        };
        if entries.is_empty() {
            return;
        }
        let position = match self.history_pos {
            Some(position) => position.saturating_sub(1),
            None => {
                self.draft = std::mem::take(&mut self.buffer);
                entries.len() - 1
            }
        };
        self.history_pos = Some(position);
        self.buffer = entries[position].clone();
        self.cursor = self.buffer.len();
    }

    /// Recall the next (newer) history entry; moving past the newest entry
    /// restores the draft saved when browsing started.
    /// 调出下一条（更新的）历史输入；越过最新条目时恢复开始浏览时保存的
    /// 草稿。
    fn recall_next(&mut self) {
        let Some(position) = self.history_pos else {
            return;
        };
        let Some(entries) = self.history_entries() else {
            return;
        };
        if position + 1 < entries.len() {
            self.history_pos = Some(position + 1);
            self.buffer = entries[position + 1].clone();
        } else {
            self.history_pos = None;
            self.buffer = std::mem::take(&mut self.draft);
        }
        self.cursor = self.buffer.len();
    }

    /// Snapshot of the shared history entries, or `None` without a history.
    /// 共享历史条目的快照；无历史时返回 `None`。
    fn history_entries(&self) -> Option<Vec<String>> {
        self.history.as_ref().map(|history| history.snapshot())
    }

    /// Fill the active token with the selected candidate, if any.
    /// Returns `true` when a candidate was applied.
    /// 如有选中候选，把活动 token 替换为该候选；返回是否应用了候选。
    fn apply_selected(&mut self) -> bool {
        let Some((trigger, start, end)) = self.active_token_range() else {
            return false;
        };
        let Some(candidate) = self.candidates.get(self.selected) else {
            return false;
        };
        // Commands already carry `/`, but skill candidates are stable
        // unique names such as `project-.agents-demo`; prepend `/` so the
        // submitted line always starts with the slash and the loop dispatches
        // it as a command.
        // 命令候选自带 `/`，但技能候选是 `project-.agents-demo` 这类稳定
        // 唯一名；补 `/` 保证提交行以斜杠开头，loop 才能按命令分派。路径
        // 候选是裸相对路径，需补回 `@`。
        let mut replacement = match trigger {
            Trigger::Command => {
                if candidate.label.starts_with('/') {
                    candidate.label.clone()
                } else {
                    format!("/{}", candidate.label)
                }
            }
            Trigger::Path => format!("@{}", candidate.label),
        };
        if trigger == Trigger::Path && candidate.is_dir {
            replacement.push(std::path::MAIN_SEPARATOR);
        }
        self.buffer.replace_range(start..end, &replacement);
        self.cursor = start + replacement.len();
        true
    }

    /// Locate the active completion token as a buffer range, from the token
    /// start up to the cursor.
    /// 定位活动补全 token 及其缓冲范围，从 token 起点到光标处。
    fn active_token_range(&self) -> Option<(Trigger, usize, usize)> {
        let end = self.cursor;
        let head = &self.buffer[..end];
        // A command token is only valid when the whole line is the token.
        // 命令 token 仅当整行就是该 token 时有效。
        if let Some(rest) = head.strip_prefix('/')
            && !rest.contains(' ')
        {
            return Some((Trigger::Command, 0, end));
        }
        // A path token at the line start, such as `@sr`.
        // 行首的路径 token，如 `@sr`。
        if head.starts_with('@') && !head[1..].contains(' ') {
            return Some((Trigger::Path, 0, end));
        }
        // A path token after a space, such as `note @sr`; only the last
        // `@token` is completed while it is still being typed.
        // 空格后的路径 token，如 `note @sr`；仅当最后一个 `@token` 仍在
        // 输入时对其补全。
        if let Some(pos) = head.rfind(" @") {
            let start = pos + 1;
            let token = &head[start..];
            if !token.contains(' ') {
                return Some((Trigger::Path, start, end));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Candidate, CompletionState, InputHistory, Key, Outcome, Trigger};

    fn candidate(label: &str) -> Candidate {
        Candidate {
            label: label.to_owned(),
            description: String::new(),
            is_dir: false,
            dimmed: false,
        }
    }

    #[test]
    fn starts_empty() {
        let state = CompletionState::new();
        assert_eq!(state.buffer(), "");
        assert_eq!(state.cursor(), 0);
        assert!(state.candidates().is_empty());
        assert_eq!(state.active_trigger(), None);
    }

    #[test]
    fn command_trigger_tracks_the_full_token() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('/'));
        state.handle_key(Key::Char('h'));
        state.handle_key(Key::Char('i'));
        assert_eq!(state.active_trigger(), Some(Trigger::Command));
        assert_eq!(state.active_query(), Some("/hi"));
    }

    #[test]
    fn command_trigger_requires_no_space() {
        let mut state = CompletionState::new();
        for ch in "/he lp".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.active_trigger(), None);
    }

    #[test]
    fn path_trigger_at_line_start_strips_at() {
        let mut state = CompletionState::new();
        for ch in "@sr".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.active_trigger(), Some(Trigger::Path));
        assert_eq!(state.active_query(), Some("sr"));
    }

    #[test]
    fn path_trigger_after_a_space() {
        let mut state = CompletionState::new();
        for ch in "note @sr".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.active_trigger(), Some(Trigger::Path));
        assert_eq!(state.active_query(), Some("sr"));
    }

    #[test]
    fn plain_text_has_no_trigger() {
        let mut state = CompletionState::new();
        for ch in "hello".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.active_trigger(), None);
        assert_eq!(state.active_query(), None);
    }

    #[test]
    fn backspace_updates_buffer_and_cursor() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('a'));
        state.handle_key(Key::Char('b'));
        state.handle_key(Key::Backspace);
        assert_eq!(state.buffer(), "a");
        assert_eq!(state.cursor(), 1);
        state.handle_key(Key::Backspace);
        state.handle_key(Key::Backspace);
        assert_eq!(state.buffer(), "");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn set_candidates_resets_stale_selection() {
        let mut state = CompletionState::new();
        state.set_candidates(vec![candidate("/help"), candidate("/history")]);
        state.selected = 1;
        state.set_candidates(vec![candidate("/help")]);
        assert_eq!(state.selected(), 0);
        state.set_candidates(Vec::new());
        assert!(state.candidates().is_empty());
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn up_and_down_are_noops_without_candidates() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Up);
        state.handle_key(Key::Down);
        assert_eq!(state.selected(), 0);
        assert!(state.candidates().is_empty());
    }

    #[test]
    fn up_and_down_wrap_around() {
        let mut state = CompletionState::new();
        state.set_candidates(vec![candidate("/help"), candidate("/history")]);
        state.handle_key(Key::Down);
        assert_eq!(state.selected(), 1);
        state.handle_key(Key::Down);
        assert_eq!(state.selected(), 0);
        state.handle_key(Key::Up);
        assert_eq!(state.selected(), 1);
        state.handle_key(Key::Up);
        assert_eq!(state.selected(), 0);
    }

    #[test]
    fn tab_fills_command_candidate() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('/'));
        state.handle_key(Key::Char('h'));
        state.set_candidates(vec![candidate("/help")]);
        state.handle_key(Key::Tab);
        assert_eq!(state.buffer(), "/help");
        assert_eq!(state.cursor(), 5);
    }

    #[test]
    fn tab_prepends_slash_to_skill_candidate() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('/'));
        state.handle_key(Key::Char('p'));
        state.set_candidates(vec![candidate("project-.agents-demo")]);
        state.handle_key(Key::Tab);
        assert_eq!(state.buffer(), "/project-.agents-demo");
    }

    #[test]
    fn tab_fills_path_candidate_with_at_prefix() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('@'));
        state.handle_key(Key::Char('s'));
        state.handle_key(Key::Char('r'));
        state.set_candidates(vec![candidate("src")]);
        state.handle_key(Key::Tab);
        assert_eq!(state.buffer(), "@src");
    }

    #[test]
    fn tab_appends_separator_to_path_directory_candidate() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('@'));
        state.handle_key(Key::Char('s'));
        let mut directory = candidate("src");
        directory.is_dir = true;
        state.set_candidates(vec![directory]);
        state.handle_key(Key::Tab);
        let expected = format!("@src{}", std::path::MAIN_SEPARATOR);
        assert_eq!(state.buffer(), expected);
    }

    #[test]
    fn path_trigger_after_a_space_stops_at_the_next_space() {
        let mut state = CompletionState::new();
        for ch in "note @sr ".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.active_trigger(), None);
        assert_eq!(state.active_query(), None);
    }

    #[test]
    fn esc_closes_suggestions_but_keeps_buffer() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('/'));
        state.handle_key(Key::Char('h'));
        state.set_candidates(vec![candidate("/help")]);
        state.handle_key(Key::Esc);
        assert!(state.candidates().is_empty());
        assert_eq!(state.buffer(), "/h");
    }

    #[test]
    fn enter_submits_the_buffer() {
        let mut state = CompletionState::new();
        for ch in "/help".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(
            state.handle_key(Key::Enter),
            Outcome::Submit("/help".to_owned())
        );
    }

    #[test]
    fn enter_completes_selected_candidate_then_submits() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('/'));
        state.handle_key(Key::Char('p'));
        state.set_candidates(vec![candidate("project-.agents-demo")]);
        assert_eq!(
            state.handle_key(Key::Enter),
            Outcome::Submit("/project-.agents-demo".to_owned())
        );
    }

    #[test]
    fn left_and_right_move_the_cursor_and_insert_mid_line() {
        let mut state = CompletionState::new();
        for ch in "ab".chars() {
            state.handle_key(Key::Char(ch));
        }
        state.handle_key(Key::Left);
        assert_eq!(state.cursor(), 1);
        state.handle_key(Key::Char('X'));
        assert_eq!(state.buffer(), "aXb");
        assert_eq!(state.cursor(), 2);
    }

    #[test]
    fn backspace_removes_the_char_before_the_cursor() {
        let mut state = CompletionState::new();
        for ch in "ab".chars() {
            state.handle_key(Key::Char(ch));
        }
        state.handle_key(Key::Left);
        state.handle_key(Key::Backspace);
        assert_eq!(state.buffer(), "b");
        assert_eq!(state.cursor(), 0);
    }

    #[test]
    fn left_at_line_start_and_right_at_line_end_are_noops() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('a'));
        state.handle_key(Key::Left);
        state.handle_key(Key::Left);
        assert_eq!(state.cursor(), 0);
        state.handle_key(Key::Right);
        state.handle_key(Key::Right);
        assert_eq!(state.cursor(), 1);
    }

    #[test]
    fn ctrl_arrows_move_by_whole_words() {
        let mut state = CompletionState::new();
        for ch in "hello world foo".chars() {
            state.handle_key(Key::Char(ch));
        }
        assert_eq!(state.buffer().len(), 15);
        state.handle_key(Key::CtrlLeft);
        assert_eq!(state.cursor(), 12);
        state.handle_key(Key::CtrlLeft);
        assert_eq!(state.cursor(), 6);
        state.handle_key(Key::CtrlLeft);
        assert_eq!(state.cursor(), 0);
        state.handle_key(Key::CtrlRight);
        assert_eq!(state.cursor(), 6);
        state.handle_key(Key::CtrlRight);
        assert_eq!(state.cursor(), 12);
        state.handle_key(Key::CtrlRight);
        assert_eq!(state.cursor(), 15);
    }

    #[test]
    fn history_up_recalls_older_entries_and_down_restores_the_draft() {
        let history = Arc::new(InputHistory::new());
        history.push("first");
        history.push("second");
        let mut state = CompletionState::with_history(history);
        state.handle_key(Key::Char('x'));
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "second");
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "first");
        state.handle_key(Key::Down);
        assert_eq!(state.buffer(), "second");
        state.handle_key(Key::Down);
        assert_eq!(state.buffer(), "x");
    }

    #[test]
    fn history_push_keeps_only_the_latest_duplicate() {
        let history = Arc::new(InputHistory::new());
        history.push("alpha");
        history.push("beta");
        history.push("alpha");
        let mut state = CompletionState::with_history(history);
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "alpha");
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "beta");
        // Only two unique entries exist, so a third Up stays on the oldest.
        // 只有两个唯一条目，第三次 Up 停留在最旧一条。
        state.handle_key(Key::Up);
        assert_eq!(state.buffer(), "beta");
    }

    #[test]
    fn up_and_down_without_history_are_noops() {
        let mut state = CompletionState::new();
        state.handle_key(Key::Char('a'));
        state.handle_key(Key::Up);
        state.handle_key(Key::Down);
        assert_eq!(state.buffer(), "a");
        assert_eq!(state.cursor(), 1);
    }
}
