//! Shared helpers for the loop handler tests.
//! loop 处理器测试的共享辅助函数。

mod copy;
mod submit;
mod summary;

use std::path::Path;
use std::sync::Arc;

use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::executor::Executor;
use manualaid_core::parser::FormatRegistry;
use manualaid_ws::session::{RoundStats, SessionLog};

/// Round index selecting the most recent (and only) round in these tests.
/// 这些测试中选择最近（且唯一）轮次的轮次索引。
const LATEST_ROUND_INDEX: &str = "1";

/// Out-of-range round index used to exercise the invalid-index message.
/// 用于触发无效索引消息的越界轮次索引。
const OUT_OF_RANGE_INDEX: &str = "9";

fn executor(root: &Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(SessionMode::AcceptEdit),
        Arc::new(None),
    )
}

fn read_call(root: &Path) -> String {
    let file = root.join("target.txt");
    std::fs::write(&file, "hello").unwrap();
    format!("<read><file_path>{}</file_path></read>", file.display())
}

async fn session_with_round(root: &Path) -> SessionLog {
    let mut session = SessionLog::new();
    add_round(root, &mut session, RoundStats::default()).await;
    session
}

async fn add_round(root: &Path, session: &mut SessionLog, stats: RoundStats) {
    let (calls, results) = read_round(root).await;
    session.push(calls, results, stats);
}

/// Add one round whose timestamp sits `offset_seconds` after a fixed
/// baseline, so the interval statistics are deterministic.
/// 添加一轮，其时间戳位于固定基准之后 `offset_seconds` 秒处，使间隔统计
/// 确定。
async fn add_round_at(
    root: &Path,
    session: &mut SessionLog,
    stats: RoundStats,
    offset_seconds: u64,
) {
    let (calls, results) = read_round(root).await;
    let at = std::time::SystemTime::UNIX_EPOCH
        + std::time::Duration::from_secs(1_700_000_000 + offset_seconds);
    session.push_at(calls, results, stats, at);
}

/// Execute one `read` round and return its parsed calls plus their results.
/// 执行一轮 `read` 调用并返回其解析调用与结果。
async fn read_round(
    root: &Path,
) -> (
    Vec<manualaid_core::parser::ParsedToolCall>,
    Vec<manualaid_core::tools::ToolResult>,
) {
    let registry = FormatRegistry::new();
    let calls = registry.parse(&read_call(root)).unwrap().calls;
    let exec = executor(root);
    let mut results = Vec::new();
    for call in &calls {
        results.push(exec.execute(call.clone()).await);
    }
    (calls, results)
}
