//! Pure state machine for the interactive completion input: key events
//! mutate the buffer and suggestion selection without touching the
//! terminal, so every transition is unit-testable without a TTY.
//! 交互式补全输入的纯状态机：按键事件只改变缓冲与建议选中状态，不接触
//! 终端，因此每个状态转移都能在无 TTY 环境下单元测试。

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
    /// A printable character to append to the buffer.
    /// 追加到缓冲的可打印字符。
    Char(char),
    Backspace,
    Up,
    Down,
    Tab,
    Esc,
    Enter,
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
    /// Cursor is currently always at the end of the buffer; the field is
    /// kept so mid-line editing can be added without changing the shape.
    /// 光标当前始终位于缓冲末尾；保留该字段是为将来支持行内编辑时无需
    /// 改变结构。
    cursor: usize,
}

impl CompletionState {
    /// Create an empty state with no suggestions.
    /// 创建无任何建议的空状态。
    pub(crate) fn new() -> Self {
        Self {
            buffer: String::new(),
            candidates: Vec::new(),
            selected: 0,
            cursor: 0,
        }
    }

    /// Current input line.
    /// 当前输入行。
    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Cursor position, always at the end of the buffer for now.
    /// 光标位置；目前始终在缓冲末尾。
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
                self.buffer.push(ch);
                self.cursor = self.buffer.len();
                Outcome::Handled
            }
            Key::Backspace => {
                if !self.buffer.is_empty() {
                    self.buffer.pop();
                    self.cursor = self.buffer.len();
                }
                Outcome::Handled
            }
            Key::Up => {
                if !self.candidates.is_empty() {
                    self.selected = if self.selected == 0 {
                        self.candidates.len() - 1
                    } else {
                        self.selected - 1
                    };
                }
                Outcome::Handled
            }
            Key::Down => {
                if !self.candidates.is_empty() {
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
        self.cursor = self.buffer.len();
        true
    }

    /// Locate the active completion token as a buffer range.
    /// 定位活动补全 token 及其缓冲范围。
    fn active_token_range(&self) -> Option<(Trigger, usize, usize)> {
        // A command token is only valid when the whole line is the token.
        // 命令 token 仅当整行就是该 token 时有效。
        if let Some(rest) = self.buffer.strip_prefix('/')
            && !rest.contains(' ')
        {
            return Some((Trigger::Command, 0, self.buffer.len()));
        }
        // A path token at the line start, such as `@sr`.
        // 行首的路径 token，如 `@sr`。
        if self.buffer.starts_with('@') && !self.buffer[1..].contains(' ') {
            return Some((Trigger::Path, 0, self.buffer.len()));
        }
        // A path token after a space, such as `note @sr`; only the last
        // `@token` is completed while it is still being typed.
        // 空格后的路径 token，如 `note @sr`；仅当最后一个 `@token` 仍在
        // 输入时对其补全。
        if let Some(pos) = self.buffer.rfind(" @") {
            let start = pos + 1;
            let token = &self.buffer[start..];
            if !token.contains(' ') {
                return Some((Trigger::Path, start, self.buffer.len()));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{Candidate, CompletionState, Key, Outcome, Trigger};

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
}
