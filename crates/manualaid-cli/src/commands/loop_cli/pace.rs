//! Copy-pace metering for the interactive loop: how long ago the last
//! prompted copy happened and how many copies fall inside the tracked time
//! windows. The clipboard is the only place where this loop can observe how
//! often the user posts to an external chat, so the meter never reads the
//! clipboard, never blocks and never delays a copy.
//! 交互式 loop 的复制节奏计量：距上次复制提示词的时长，以及各统计窗口内的
//! 复制次数。剪贴板是本 loop 唯一能观测「用户向外部聊天发送频率」的地方，
//! 因此本计量只记录写入时刻，绝不读取剪贴板，也不阻塞或延迟任何复制。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use manualaid_core::clipboard::{ClipboardProvider, CopyKind, MockClipboard, RealClipboard};

use super::utils::t_fmt;

/// Time windows reported by the pace line, shortest first. The count for a
/// window is the number of send-like copies inside it, the most recent copy
/// included.
/// 节奏行报告的统计窗口，由短到长。每个窗口的计数是窗口内「算作一次发送」
/// 的复制次数，包含最近一次。
pub(super) const WINDOWS: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(10 * 60),
    Duration::from_secs(30 * 60),
    Duration::from_secs(60 * 60),
];

/// How many of [`WINDOWS`] the pace line shows while folded; the rest appear
/// once the user expands the line from the configuration menu.
/// 折叠状态下节奏行显示 [`WINDOWS`] 的前几项；其余项在用户从配置菜单展开
/// 后显示。
pub(super) const FOLDED_WINDOWS: usize = 3;

/// Placeholder names of [`WINDOWS`], in the same order, as used by the
/// localized pace templates.
/// [`WINDOWS`] 对应的占位符名，顺序一致，供本地化节奏模板使用。
const WINDOW_NAMES: [&str; WINDOWS.len()] = ["w1", "w5", "w10", "w30", "w60"];

/// Writes older than this are dropped: nothing beyond the widest window can
/// change a reported count.
/// 早于该时长的写入会被丢弃：超出最宽窗口的记录不会再影响任何已报告的计数。
const WIDEST_WINDOW: Duration = WINDOWS[WINDOWS.len() - 1];

/// Upper bound on the retained write times. The widest window needs at most
/// one entry per copy in the last hour; the cap keeps memory bounded even if
/// something copies in a tight loop.
/// 保留的写入时刻上限。最宽窗口最多需要「过去一小时每次复制一条」；该上限
/// 使即使有东西在紧凑循环里复制，内存也有界。
const MAX_TRACKED_WRITES: usize = 4096;

/// The session's clipboard-write log.
/// 会话的剪贴板写入日志。
#[derive(Debug, Default)]
pub(super) struct CopyLedger {
    /// Times of the send-like copies inside [`WIDEST_WINDOW`], oldest first.
    /// [`WIDEST_WINDOW`] 内「算作一次发送」的复制时刻，最旧在前。
    writes: VecDeque<Instant>,
    /// Time of the most recent send-like copy.
    /// 最近一次「算作一次发送」的复制时刻。
    last_at: Option<Instant>,
    /// Time of the send-like copy before [`CopyLedger::last_at`].
    /// [`CopyLedger::last_at`] 之前一次「算作一次发送」的复制时刻。
    previous_at: Option<Instant>,
    /// Lifetime count of copied system prompts.
    /// 系统提示词复制次数（会话累计）。
    system_prompt: u64,
    /// Lifetime count of copied prompt snippets.
    /// 提示词片段复制次数（会话累计）。
    snippet: u64,
    /// Lifetime count of copied tool results.
    /// 工具结果复制次数（会话累计）。
    result: u64,
}

impl CopyLedger {
    /// Record one successful write. Tool-call templates are not recorded:
    /// they are typed inside a message and never sent as a message of their
    /// own.
    /// 记录一次成功写入。工具调用模板不记录：它是写进消息内部的，从不作为
    /// 一条消息单独发送。
    pub(super) fn record(&mut self, kind: CopyKind, at: Instant) {
        if !kind.counts_as_send() {
            return;
        }
        self.previous_at = self.last_at;
        self.last_at = Some(at);
        match kind {
            CopyKind::SystemPrompt => self.system_prompt += 1,
            CopyKind::Prompt => self.snippet += 1,
            CopyKind::Result => self.result += 1,
            CopyKind::Template => return,
        }
        self.writes.push_back(at);
        self.prune(at);
    }

    /// Drop writes that fell out of the widest window, then enforce the hard
    /// cap.
    /// 丢弃已滑出最宽窗口的写入，然后执行硬上限。
    fn prune(&mut self, now: Instant) {
        let oldest = now.checked_sub(WIDEST_WINDOW).unwrap_or(now);
        while self.writes.front().is_some_and(|at| *at < oldest) {
            self.writes.pop_front();
        }
        while self.writes.len() > MAX_TRACKED_WRITES {
            self.writes.pop_front();
        }
    }

    /// Read the meter as it stands at `now`.
    /// 读取 `now` 时刻的计量状态。
    pub(super) fn report(&self, now: Instant) -> PaceReport {
        let gap = match (self.last_at, self.previous_at) {
            (Some(last), Some(previous)) => last.checked_duration_since(previous),
            _ => None,
        };
        PaceReport {
            gap,
            last_at: self.last_at,
            windows: WINDOWS.map(|window| {
                let since = now.checked_sub(window).unwrap_or(now);
                self.writes.iter().filter(|at| **at >= since).count()
            }),
            system_prompt: self.system_prompt,
            snippet: self.snippet,
            result: self.result,
        }
    }
}

/// One reading of the copy meter.
/// 复制计量的一次读数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PaceReport {
    /// Time between the two most recent send-like copies; `None` until the
    /// second one happens.
    /// 最近两次「算作一次发送」的复制之间的时长；第二次发生前为 `None`。
    pub(super) gap: Option<Duration>,
    /// Time of the most recent send-like copy; `None` until the first one.
    /// 最近一次「算作一次发送」的复制时刻；第一次发生前为 `None`。
    pub(super) last_at: Option<Instant>,
    /// Copy counts per [`WINDOWS`] entry, in the same order.
    /// 与 [`WINDOWS`] 逐项对应的复制次数。
    pub(super) windows: [usize; WINDOWS.len()],
    /// Lifetime count of copied system prompts.
    /// 系统提示词复制次数。
    pub(super) system_prompt: u64,
    /// Lifetime count of copied prompt snippets.
    /// 提示词片段复制次数。
    pub(super) snippet: u64,
    /// Lifetime count of copied tool results.
    /// 工具结果复制次数。
    pub(super) result: u64,
}

impl PaceReport {
    /// Copies of prompts (the system prompt included) and of tool results
    /// together: the writes that each became one post to an external chat.
    /// 提示词（含系统提示词）与工具结果的复制次数合计：这些写入各自成为向
    /// 外部聊天的一次发送。
    pub fn send_copies(&self) -> u64 {
        self.system_prompt + self.snippet + self.result
    }
}

/// Read side of the copy meter. A provider that does not track writes
/// reports an empty reading, so callers can format a report without asking
/// which provider they hold.
/// 复制计量的读取侧。不跟踪写入的 provider 返回空读数，调用方无需区分持有
/// 哪种 provider 即可格式化读数。
pub(super) trait PaceSource {
    /// Read the meter as it stands right now.
    /// 读取此刻的计量状态。
    fn pace(&self) -> PaceReport;
}

impl PaceSource for RealClipboard {
    fn pace(&self) -> PaceReport {
        PaceReport::default()
    }
}

impl PaceSource for MockClipboard {
    fn pace(&self) -> PaceReport {
        PaceReport::default()
    }
}

/// A clipboard provider that records every successful write in a shared
/// [`CopyLedger`] before handing it to the wrapped provider.
/// 把每次成功写入记入共享 [`CopyLedger`] 后再交给被包裹 provider 的剪贴板
/// provider。
#[derive(Debug)]
pub(super) struct PacedClipboard<P> {
    inner: P,
    ledger: Arc<Mutex<CopyLedger>>,
}

impl<P: ClipboardProvider> PacedClipboard<P> {
    /// Wrap `inner`, recording into the ledger shared with the caller.
    /// 包裹 `inner`，并把写入记入与调用方共享的 ledger。
    pub(super) fn new(inner: P, ledger: Arc<Mutex<CopyLedger>>) -> Self {
        Self { inner, ledger }
    }

    /// Write through `inner` and record the write only once it succeeded: a
    /// failed write posts nothing, so it must not count as a post.
    /// 经 `inner` 写入，且仅在写入成功后记录：写入失败没有发出任何东西，
    /// 因此不能算作一次发送。
    fn record(&self, kind: CopyKind, text: &str) -> Result<(), String> {
        self.inner.write_kind(kind, text)?;
        self.ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .record(kind, Instant::now());
        Ok(())
    }
}

impl<P: ClipboardProvider> ClipboardProvider for PacedClipboard<P> {
    fn read(&self) -> Result<String, String> {
        self.inner.read()
    }

    /// Fallback for a write whose caller did not state a kind. Counting it
    /// as a prompt snippet keeps the meter from silently under-reporting
    /// posts; the loop states the kind at every write site it controls.
    /// 调用方未声明种类的写入的兜底。按提示词片段计数可避免计量悄悄少报
    /// 发送次数；本 loop 控制的每处写入都会声明种类。
    fn write(&self, text: &str) -> Result<(), String> {
        self.record(CopyKind::Prompt, text)
    }

    fn write_kind(&self, kind: CopyKind, text: &str) -> Result<(), String> {
        self.record(kind, text)
    }
}

impl<P: ClipboardProvider> PaceSource for PacedClipboard<P> {
    fn pace(&self) -> PaceReport {
        self.ledger
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .report(Instant::now())
    }
}

/// Render the pace block: the gap since the previous copy, the counts per
/// window, and — once per session while folded — the hint that the remaining
/// windows can be expanded.
/// 渲染节奏区块：距上次复制的时长、各窗口的次数，以及（折叠状态下每会话
/// 一次）其余窗口可展开的提示。
pub(super) fn pace_lines(report: &PaceReport, expanded: bool, show_fold_hint: bool) -> Vec<String> {
    let values: [String; WINDOWS.len()] = report.windows.map(|count| count.to_string());
    let visible = if expanded {
        WINDOWS.len()
    } else {
        FOLDED_WINDOWS
    };
    let counts: Vec<(&str, &str)> = WINDOW_NAMES
        .iter()
        .zip(values.iter())
        .take(visible)
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let mut lines = Vec::new();
    if let Some(gap) = report.gap {
        // The timestamp belongs to the copy that was just made, so the line
        // anchors the cadence to the clock the external chat also sees.
        // 该时刻属于刚刚完成的那次复制，因此这一行把节奏锚定到外部聊天也能
        // 看到的钟点上。
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        lines.push(t_fmt(
            "cli.loop.pace_gap",
            &[("elapsed", &crate::format_span(gap)), ("time", &time)],
        ));
    }
    let key = if expanded {
        "cli.loop.pace_windows_expanded"
    } else {
        "cli.loop.pace_windows"
    };
    lines.push(t_fmt(key, &counts));
    if show_fold_hint {
        lines.push(i18n::t_str("cli.loop.pace_fold_hint"));
    }
    lines
}

#[cfg(test)]
#[path = "pace_tests.rs"]
mod tests;
