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
#[path = "api/terminal_title.rs"]
mod terminal_title;

use std::sync::{Mutex, MutexGuard};

use manualaid_cli::style;

/// Serializes tests that depend on the process-wide i18n locale.
/// 串行化依赖进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Serializes tests that depend on the process-wide styling switch.
/// 串行化依赖进程级样式开关的测试。
static STYLE_LOCK: Mutex<()> = Mutex::new(());

/// Restores the process-wide locale to `en` on drop.
/// 在 drop 时将进程级 locale 恢复为 `en`。
struct LocaleGuard {
    _lock: MutexGuard<'static, ()>,
}

fn locale_guard() -> LocaleGuard {
    LocaleGuard {
        _lock: LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    }
}

impl Drop for LocaleGuard {
    fn drop(&mut self) {
        i18n::set_locale("en");
    }
}

/// Restores the process-wide styling switch to its previous value on drop.
/// 在 drop 时将进程级样式开关恢复为进入前的值。
struct StyleGuard {
    _lock: MutexGuard<'static, ()>,
    original: bool,
}

fn style_guard() -> StyleGuard {
    let _lock = STYLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    StyleGuard {
        _lock,
        original: style::is_enabled(),
    }
}

impl Drop for StyleGuard {
    fn drop(&mut self) {
        style::set_enabled(self.original);
    }
}
