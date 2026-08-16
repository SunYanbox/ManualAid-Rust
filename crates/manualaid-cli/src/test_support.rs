//! Test-only helpers shared by the unit tests under `src/`.
//! `src/` 下单元测试共享的测试辅助。

use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
pub(crate) static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the locale lock, recovering from poison if a previous test
/// panicked while holding it. Calling `unwrap()` directly would abort the
/// entire test run on the first poisoning event.
/// 获取 locale 锁，并在前一个测试 panic 导致锁中毒时进行恢复。直接调用
/// `unwrap()` 会在首次中毒事件时中止整个测试运行。
pub(crate) fn acquire_locale_lock() -> std::sync::MutexGuard<'static, ()> {
    LOCALE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Serializes tests that reload the global skill store with a temp home.
/// 串行化用临时主目录重载全局技能库的测试。
pub(crate) static SKILL_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that change the process working directory.
/// 串行化会修改进程工作目录的测试。
pub(crate) static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that mutate the process-wide styling switch.
/// 串行化修改进程级样式开关的测试。
pub(crate) static STYLE_LOCK: Mutex<()> = Mutex::new(());

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
