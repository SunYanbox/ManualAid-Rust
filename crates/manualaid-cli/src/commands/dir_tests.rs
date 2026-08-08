//! Unit tests for the crate-private confirmation helper in `dir.rs`.
//! `dir.rs` 中 crate 私有确认辅助函数的单元测试。

use std::path::PathBuf;

use super::{clean_manualaid_dirs, confirm_or_abort, run_dir_clean_with_stdin, run_dir_view};
use crate::test_support::{CWD_LOCK, LOCALE_LOCK, temp_dir};

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

#[test]
fn clean_project_scope_uses_cwd_as_base() {
    let _cwd = CWD_LOCK.lock().unwrap();
    let original = std::env::current_dir().unwrap();
    let project = temp_dir("clean-project");
    std::env::set_current_dir(&project).unwrap();
    std::fs::create_dir_all(project.join(".ManualAid")).unwrap();
    std::fs::write(project.join(".ManualAid").join("config.toml"), "x").unwrap();
    let home = temp_dir("clean-project-home");
    run_dir_clean_with_stdin(true, false, true, &home, false, &mut "".as_bytes()).unwrap();
    assert!(!project.join(".ManualAid").exists());
    std::env::set_current_dir(&original).unwrap();
    let _ = std::fs::remove_dir_all(&project);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn clean_global_scope_confirm_yes_removes_home_dir() {
    let home = temp_dir("clean-confirm-yes");
    std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
    let mut stdin = "y\n".as_bytes();
    run_dir_clean_with_stdin(false, true, false, &home, true, &mut stdin).unwrap();
    assert!(!home.join(".ManualAid").exists());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn clean_global_scope_confirm_no_aborts() {
    let _lock = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let home = temp_dir("clean-confirm-no");
    std::fs::create_dir_all(home.join(".ManualAid")).unwrap();
    let mut stdin = "n\n".as_bytes();
    let err = run_dir_clean_with_stdin(false, true, false, &home, true, &mut stdin).unwrap_err();
    assert!(err.contains("Aborted"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn clean_manualaid_dirs_reports_missing_dirs() {
    let home = temp_dir("clean-missing");
    let lines = clean_manualaid_dirs(&[home.clone()]).unwrap();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("does not exist") || lines[0].contains("不存在"));
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn dir_view_with_real_home_falls_back_to_home_dir() {
    let _lock = LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    assert!(run_dir_view(false, true, None, None, None).is_ok());
}
