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
