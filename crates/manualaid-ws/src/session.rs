//! In-memory session batch records: each executed round stores its parsed
//! calls and results so the CLI loop can copy the i-th latest batch.
//! 内存会话批次记录：每一轮执行保存其解析调用与结果，使 CLI loop 可以
//! 复制第 i 个最新批次。

use manualaid_core::parser::ParsedToolCall;
use manualaid_core::tools::ToolResult;

/// Statistics of one executed round.
/// 一轮执行的统计信息。
#[derive(Debug, Clone, Copy, Default)]
pub struct RoundStats {
    /// Parameter parsing duration in milliseconds (registry.parse).
    /// 参数解析耗时（毫秒，registry.parse）。
    pub parse_duration_ms: u64,
    /// Audit duration in milliseconds: pre-checks plus user approval
    /// waiting. Execution time is NOT included.
    /// 审计耗时（毫秒）：预检加上用户审批等待。不含执行时间。
    pub audit_duration_ms: u64,
    /// Total execution duration in milliseconds (sum of all tool
    /// executions; excludes audit time).
    /// 总执行耗时（毫秒，所有工具执行之和；不含审计时间）。
    pub total_execution_duration_ms: u64,
    /// Total estimated token consumption of the round (input + outputs).
    /// 本轮估算 Token 消耗总量（输入 + 输出）。
    pub total_tokens: u64,
}

/// One executed round: the parsed calls, their results and stats.
/// 一轮已执行的解析调用、结果与统计。
#[derive(Debug, Clone)]
pub struct BatchRecord {
    /// The parsed tool calls of this round.
    /// 本轮解析出的工具调用。
    pub calls: Vec<ParsedToolCall>,
    /// The results of this round, in call order.
    /// 本轮结果（按调用顺序）。
    pub results: Vec<ToolResult>,
    /// Timing and token statistics of this round.
    /// 本轮耗时与 Token 统计。
    pub stats: RoundStats,
}

/// In-memory footprint of the persisted session structures, broken down by
/// category. Values are estimates: `IndexMap`'s internal hash storage is
/// not publicly measurable, so only key/value heap bytes are counted.
/// 会话持久结构的内存占用，按类别拆分。数值为估算：`IndexMap` 的内部
/// 哈希存储不可公开测量，因此只统计键/值的堆内存字节数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemoryUsage {
    /// Total bytes across all categories.
    /// 所有类别合计字节数。
    pub total_bytes: u64,
    /// Bytes held by the parsed-call structures (strings/vectors).
    /// 解析调用结构（字符串/向量）占用的字节数。
    pub calls_bytes: u64,
    /// Bytes held by the tool-result structures (strings/vectors).
    /// 工具结果结构（字符串/向量）占用的字节数。
    pub results_bytes: u64,
    /// Bytes held by stats and the record/log scaffolding.
    /// 统计信息与记录/日志框架占用的字节数。
    pub metadata_bytes: u64,
}

/// Append-only in-memory log of executed rounds.
/// 已执行轮次的只追加内存日志。
#[derive(Debug, Default)]
pub struct SessionLog {
    rounds: Vec<BatchRecord>,
}

impl SessionLog {
    /// Maximum number of rounds retained in memory; older rounds are
    /// dropped on push.
    /// 内存中保留的最大轮次数；更旧的轮次在 push 时丢弃。
    pub const MAX_ROUNDS: usize = 200;

    /// Create an empty session log.
    /// 创建空的会话日志。
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one executed round, dropping the oldest rounds beyond
    /// [`SessionLog::MAX_ROUNDS`].
    /// 追加一轮已执行记录，超出 [`SessionLog::MAX_ROUNDS`] 时丢弃最旧的
    /// 轮次。
    pub fn push(
        &mut self,
        calls: Vec<ParsedToolCall>,
        results: Vec<ToolResult>,
        stats: RoundStats,
    ) {
        self.rounds.push(BatchRecord {
            calls,
            results,
            stats,
        });
        if self.rounds.len() > Self::MAX_ROUNDS {
            let excess = self.rounds.len() - Self::MAX_ROUNDS;
            self.rounds.drain(0..excess);
        }
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

    /// All recorded rounds, oldest first. Render newest-first by iterating
    /// in reverse.
    /// 所有已记录轮次（最旧在前）。渲染最新在前时反向迭代。
    pub fn rounds(&self) -> &[BatchRecord] {
        &self.rounds
    }

    /// Total tool-call count across all rounds.
    /// 所有轮次的工具调用总数。
    pub fn total_calls(&self) -> usize {
        self.rounds.iter().map(|round| round.results.len()).sum()
    }

    /// Estimated in-memory footprint of the retained records.
    /// 保留记录的估算内存占用。
    pub fn memory_usage(&self) -> MemoryUsage {
        let mut calls_bytes = 0usize;
        let mut results_bytes = 0usize;
        let mut metadata_bytes = size_of::<SessionLog>();
        for round in &self.rounds {
            calls_bytes += round
                .calls
                .iter()
                .map(|call| {
                    size_of::<ParsedToolCall>()
                        + call.tool_name.capacity()
                        + call
                            .params
                            .iter()
                            .map(|(key, value)| key.capacity() + value_heap_bytes(value))
                            .sum::<usize>()
                })
                .sum::<usize>();
            results_bytes += round
                .results
                .iter()
                .map(|result| {
                    size_of::<ToolResult>()
                        + result.tool_name.capacity()
                        + result.output.capacity()
                        + result.params_summary.capacity()
                        + result
                            .audit_decisions
                            .iter()
                            .map(|(param, _)| {
                                size_of::<(String, manualaid_core::audit::AuditDecision)>()
                                    + param.capacity()
                            })
                            .sum::<usize>()
                })
                .sum::<usize>();
            metadata_bytes += size_of::<BatchRecord>() + size_of::<RoundStats>();
        }
        MemoryUsage {
            total_bytes: (calls_bytes + results_bytes + metadata_bytes) as u64,
            calls_bytes: calls_bytes as u64,
            results_bytes: results_bytes as u64,
            metadata_bytes: metadata_bytes as u64,
        }
    }
}

/// Heap bytes held by a JSON value (strings, arrays, objects), excluding
/// stack size.
/// JSON 值占用的堆内存字节数（字符串、数组、对象），不含栈大小。
fn value_heap_bytes(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::String(text) => text.capacity(),
        serde_json::Value::Array(items) => items.iter().map(value_heap_bytes).sum(),
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, value)| key.capacity() + value_heap_bytes(value))
            .sum(),
        _ => 0,
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
        log.push(
            Vec::new(),
            vec![ToolResult::success("read", "1", true)],
            RoundStats::default(),
        );
        log.push(
            Vec::new(),
            vec![ToolResult::success("read", "2", true)],
            RoundStats::default(),
        );
        assert_eq!(log.latest(1).unwrap().results[0].output, "2");
        assert_eq!(log.latest(2).unwrap().results[0].output, "1");
        assert!(log.latest(0).is_none());
        assert!(log.latest(3).is_none());
    }

    #[test]
    fn len_and_total_calls_track_rounds() {
        let mut log = SessionLog::new();
        assert!(log.is_empty());
        log.push(Vec::new(), sample_results(), RoundStats::default());
        log.push(
            Vec::new(),
            vec![
                ToolResult::success("read", "a", true),
                ToolResult::failure("edit", "b"),
            ],
            RoundStats::default(),
        );
        assert_eq!(log.len(), 2);
        assert_eq!(log.total_calls(), 3);
    }

    #[test]
    fn push_truncates_to_max_rounds() {
        let mut log = SessionLog::new();
        for i in 0..SessionLog::MAX_ROUNDS + 5 {
            log.push(
                Vec::new(),
                vec![ToolResult::success("read", i.to_string(), true)],
                RoundStats::default(),
            );
        }
        assert_eq!(log.len(), SessionLog::MAX_ROUNDS);
        // The 5 oldest rounds were dropped; the newest is the latest(1).
        // 最早的 5 轮被丢弃；最新一轮仍可从 latest(1) 取到。
        assert!(log.latest(1).unwrap().results[0].output.ends_with("204"));
        assert!(
            log.latest(SessionLog::MAX_ROUNDS).unwrap().results[0]
                .output
                .ends_with("5")
        );
    }

    #[test]
    fn rounds_accessor_returns_all() {
        let mut log = SessionLog::new();
        log.push(
            Vec::new(),
            vec![ToolResult::success("read", "1", true)],
            RoundStats::default(),
        );
        log.push(
            Vec::new(),
            vec![ToolResult::success("read", "2", true)],
            RoundStats::default(),
        );
        assert_eq!(log.rounds().len(), 2);
        assert_eq!(log.rounds()[0].results[0].output, "1");
        assert_eq!(log.rounds()[1].results[0].output, "2");
    }

    #[test]
    fn memory_usage_is_consistent() {
        let log = SessionLog::new();
        let empty = log.memory_usage();
        // An empty log still holds the SessionLog scaffolding itself.
        // 空日志也包含 SessionLog 自身的框架开销。
        assert_eq!(empty.calls_bytes, 0);
        assert_eq!(empty.results_bytes, 0);
        assert_eq!(empty.total_bytes, empty.metadata_bytes);
        assert!(empty.metadata_bytes > 0);
        let mut log = SessionLog::new();
        let stats = RoundStats {
            total_tokens: 42,
            ..RoundStats::default()
        };
        let calls = manualaid_core::parser::FormatRegistry::new()
            .parse("<read><file_path>/tmp/a.txt</file_path></read>")
            .unwrap()
            .calls;
        log.push(
            calls,
            vec![ToolResult::success("read", "hello world", true)],
            stats,
        );
        let usage = log.memory_usage();
        assert!(usage.calls_bytes > 0);
        assert!(usage.results_bytes > 0);
        assert!(usage.metadata_bytes > 0);
        assert_eq!(
            usage.total_bytes,
            usage.calls_bytes + usage.results_bytes + usage.metadata_bytes
        );
    }
}
