//! Test-only helpers shared by the unit tests under `src/`.
//! `src/` 下单元测试共享的测试辅助。

use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
pub(crate) static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Create a unique temporary directory for a unit test.
/// 为单元测试创建唯一的临时目录。
pub(crate) fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "manualaid-cli-src-test-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
