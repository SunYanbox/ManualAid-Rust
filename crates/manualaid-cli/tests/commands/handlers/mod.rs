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

/// Restores the process-wide styling switch to its previous value on drop.
/// 在 drop 时将进程级样式开关恢复为进入前的值。
struct StyleGuard {
    original: bool,
}

impl StyleGuard {
    fn new() -> Self {
        Self {
            original: manualaid_cli::style::is_enabled(),
        }
    }
}

impl Drop for StyleGuard {
    fn drop(&mut self) {
        manualaid_cli::style::set_enabled(self.original);
    }
}

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
    let registry = FormatRegistry::new();
    let calls = registry.parse(&read_call(root)).unwrap().calls;
    let exec = executor(root);
    let mut results = Vec::new();
    for call in &calls {
        results.push(exec.execute(call.clone()).await);
    }
    session.push(calls, results, stats);
}
