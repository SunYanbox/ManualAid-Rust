use std::sync::{Mutex, MutexGuard};

use i18n::{changelog_all, changelog_version, changelog_versions, set_locale, t_str};

/// Serializes tests that mutate the process-wide i18n locale.
/// 串行化修改进程级 i18n locale 的测试。
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

struct LocaleGuard {
    _guard: MutexGuard<'static, ()>,
}

fn locale_guard() -> LocaleGuard {
    LocaleGuard {
        _guard: LOCALE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    }
}

impl Drop for LocaleGuard {
    fn drop(&mut self) {
        set_locale("en");
    }
}

#[test]
fn empty_changelog_has_no_versions() {
    let _guard = locale_guard();
    set_locale("zh-CN");
    assert!(changelog_versions().is_empty());
}

#[test]
fn empty_changelog_returns_empty_text_and_no_version_body() {
    let _guard = locale_guard();
    set_locale("zh-CN");
    assert_eq!(changelog_all(), "");
    assert_eq!(changelog_version("0.7.0"), None);
}

#[test]
fn set_locale_wrapper_switches_translations() {
    let _guard = locale_guard();
    set_locale("en");
    assert_eq!(t_str("exists"), "exists");
    set_locale("zh-CN");
    assert_eq!(t_str("exists"), "存在");
}
