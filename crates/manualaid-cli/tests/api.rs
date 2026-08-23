//! Integration tests for the `manualaid-cli` library API.
//! `manualaid-cli` 库公共 API 的集成测试。

mod common;

#[path = "api/dir_tree.rs"]
mod dir_tree;
#[path = "api/format.rs"]
mod format;
#[path = "api/mask.rs"]
mod mask;
#[path = "api/skill.rs"]
mod skill;

use std::sync::{Mutex, MutexGuard};

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that depend on the process-wide styling switch.
/// 串行化依赖进程级样式开关的测试。
static STYLE_LOCK: Mutex<()> = Mutex::new(());

fn locale_guard() -> MutexGuard<'static, ()> {
    LOCALE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn style_guard() -> MutexGuard<'static, ()> {
    STYLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
