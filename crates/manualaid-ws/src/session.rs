//! In-memory session batch records: each executed round stores its parsed
//! calls and results so the CLI loop can copy the i-th latest batch.
//! 内存会话批次记录：每一轮执行保存其解析调用与结果，使 CLI loop 可以
//! 复制第 i 个最新批次。

use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::ToolResult;

/// One executed round: the parsed calls and their results.
/// 一轮已执行的解析调用与其结果。
#[derive(Debug, Clone)]
pub struct BatchRecord {
    /// The parsed tool calls of this round.
    /// 本轮解析出的工具调用。
    pub calls: Vec<ParsedToolCall>,
    /// The results of this round, in call order.
    /// 本轮结果（按调用顺序）。
    pub results: Vec<ToolResult>,
}

/// Append-only in-memory log of executed rounds.
/// 已执行轮次的只追加内存日志。
#[derive(Debug, Default)]
pub struct SessionLog {
    rounds: Vec<BatchRecord>,
}

impl SessionLog {
    /// Create an empty session log.
    /// 创建空的会话日志。
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one executed round.
    /// 追加一轮已执行记录。
    pub fn push(&mut self, calls: Vec<ParsedToolCall>, results: Vec<ToolResult>) {
        self.rounds.push(BatchRecord { calls, results });
    }

    /// The number of recorded rounds.
    /// 已记录轮次的数量。
    pub fn len(&self) -> usize {
        self.rounds.len()
    }

    /// Whether no round has been recorded yet.
    /// 是否尚未记录任何轮次。
    pub fn is_empty(&self) -> bool {
        self.rounds.is_empty()
    }

    /// Return the `index`-th latest round (`1` = the most recent one).
    /// 返回从最新算起的第 `index` 轮（`1` = 最近一轮）。
    pub fn latest(&self, index: usize) -> Option<&BatchRecord> {
        if index == 0 || index > self.rounds.len() {
            return None;
        }
        self.rounds.get(self.rounds.len() - index)
    }

    /// Total tool-call count across all rounds.
    /// 所有轮次的工具调用总数。
    pub fn total_calls(&self) -> usize {
        self.rounds.iter().map(|round| round.results.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_results() -> Vec<ToolResult> {
        vec![ToolResult::success("read", "a", true)]
    }

    #[test]
    fn latest_returns_one_based_from_newest() {
        let mut log = SessionLog::new();
        log.push(Vec::new(), vec![ToolResult::success("read", "1", true)]);
        log.push(Vec::new(), vec![ToolResult::success("read", "2", true)]);
        assert_eq!(log.latest(1).unwrap().results[0].output, "2");
        assert_eq!(log.latest(2).unwrap().results[0].output, "1");
        assert!(log.latest(0).is_none());
        assert!(log.latest(3).is_none());
    }

    #[test]
    fn len_and_total_calls_track_rounds() {
        let mut log = SessionLog::new();
        assert!(log.is_empty());
        log.push(Vec::new(), sample_results());
        log.push(
            Vec::new(),
            vec![
                ToolResult::success("read", "a", true),
                ToolResult::failure("edit", "b"),
            ],
        );
        assert_eq!(log.len(), 2);
        assert_eq!(log.total_calls(), 3);
    }
}
