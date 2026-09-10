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
fn changelog_has_unreleased_and_version_entries() {
    let _guard = locale_guard();
    set_locale("zh-CN");
    let versions = changelog_versions();
    assert!(!versions.is_empty());
    assert!(versions.iter().any(|entry| entry.version == "Unreleased"));
    assert!(versions.iter().any(|entry| entry.version == "0.7.0"));
}

#[test]
fn changelog_all_contains_version_headings() {
    let _guard = locale_guard();
    set_locale("zh-CN");
    let text = changelog_all();
    assert!(text.contains("## [Unreleased]"));
    assert!(text.contains("## [0.7.0]"));
    assert!(changelog_version("0.7.0").is_some());
    assert!(changelog_version("bogus").is_none());
}

#[test]
fn changelog_switches_between_chinese_and_english() {
    let _guard = locale_guard();

    set_locale("zh-CN");
    assert!(changelog_all().contains("# 更新日志"));
    assert!(changelog_version("0.7.0").unwrap().contains("工具结果"));

    set_locale("en");
    assert!(changelog_all().contains("# Changelog"));
    assert!(changelog_version("0.7.0").unwrap().contains("Tool results"));
}

#[test]
fn changelog_falls_back_to_chinese_for_unknown_locales() {
    let _guard = locale_guard();

    set_locale("fr");
    assert!(changelog_all().contains("# 更新日志"));
    assert!(changelog_version("0.7.0").unwrap().contains("工具结果"));
}

#[test]
fn set_locale_wrapper_switches_translations() {
    let _guard = locale_guard();
    set_locale("en");
    assert_eq!(t_str("exists"), "exists");
    set_locale("zh-CN");
    assert_eq!(t_str("exists"), "存在");
}
