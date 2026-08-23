use std::sync::{Mutex, MutexGuard};

use i18n::{set_locale, t_str};

/// Serializes tests that mutate the process-wide i18n locale.
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

/// Restores the process-wide locale to `en` on drop.
/// 在 drop 时将进程级 locale 恢复为 `en`。
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
fn test_change_locale() {
    let _guard = locale_guard();
    set_locale("en");
    assert_eq!(t_str("exists"), "exists");
    set_locale("zh-CN");
    assert_eq!(t_str("exists"), "存在");
}

#[test]
fn test_not_equal() {
    let _guard = locale_guard();
    set_locale("en");
    assert_eq!(t_str("exists"), "exists");
    assert_ne!(t_str("exists"), "exist")
}

#[test]
fn test_not_exist_key() {
    let _guard = locale_guard();
    set_locale("en");
    let key = "33550336-114514-1%";
    assert_eq!(t_str(key), key);
}

#[test]
fn test_empty_key() {
    let _guard = locale_guard();
    set_locale("en");
    // 测试空字符串作为 key
    assert_eq!(t_str(""), "");
}

#[test]
fn test_special_characters() {
    let _guard = locale_guard();
    set_locale("en");
    // Missing keys are passed through verbatim, including special characters.
    // 缺失键按原样透传，包括特殊字符。
    assert_eq!(t_str("key_with_!@#$%"), "key_with_!@#$%");
}
