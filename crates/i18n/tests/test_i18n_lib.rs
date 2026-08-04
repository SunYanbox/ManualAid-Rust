use std::sync::{Mutex, MutexGuard};

use i18n::{set_locale, t_str};

/// Serializes tests that mutate the process-wide i18n locale.
static LOCALE_LOCK: Mutex<()> = Mutex::new(());

fn locale_guard() -> MutexGuard<'static, ()> {
    LOCALE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
    // 测试包含特殊字符的 key
    let result = t_str("key_with_!@#$%");
    assert!(!result.is_empty());
}
