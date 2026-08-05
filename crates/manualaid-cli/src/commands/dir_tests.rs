//! Unit tests for the crate-private confirmation helper in `dir.rs`.
//! `dir.rs` 中 crate 私有确认辅助函数的单元测试。

use std::path::PathBuf;

use super::confirm_or_abort;
use crate::test_support::{LOCALE_LOCK, temp_dir};

#[test]
fn confirm_or_abort_accepts_yes_flag() {
    let target = PathBuf::from("unused/.ManualAid");
    assert!(confirm_or_abort(&[target], true, false, || Ok(String::new())).is_ok());
}

#[test]
fn confirm_or_abort_skips_when_nothing_exists() {
    let dir = temp_dir("confirm-missing");
    let target = dir.join(".ManualAid");
    assert!(confirm_or_abort(&[target], false, false, || Ok(String::new())).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirm_or_abort_requires_terminal_without_yes() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = temp_dir("confirm-terminal");
    std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
    let err =
        confirm_or_abort(&[dir.join(".ManualAid")], false, false, || Ok("y".into())).unwrap_err();
    assert!(err.contains("Refusing to clean"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirm_or_abort_proceeds_on_yes_answer() {
    let dir = temp_dir("confirm-yes");
    std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
    assert!(confirm_or_abort(&[dir.join(".ManualAid")], false, true, || Ok("y\n".into())).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirm_or_abort_aborts_on_non_yes_answer() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let dir = temp_dir("confirm-abort");
    std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
    let err =
        confirm_or_abort(&[dir.join(".ManualAid")], false, true, || Ok("n".into())).unwrap_err();
    assert!(err.contains("Aborted"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn confirm_or_abort_surfaces_read_errors() {
    let dir = temp_dir("confirm-read-error");
    std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
    let err = confirm_or_abort(&[dir.join(".ManualAid")], false, true, || {
        Err(std::io::Error::other("boom"))
    })
    .unwrap_err();
    assert!(err.contains("boom"));
    let _ = std::fs::remove_dir_all(&dir);
}
