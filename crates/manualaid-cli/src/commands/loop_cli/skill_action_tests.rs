use super::*;

use manualaid_core::audit::{Auditor, SessionMode};
use manualaid_core::clipboard::MockClipboard;
use manualaid_core::executor::Executor;
use manualaid_ws::session::SessionLog;
use std::sync::Arc;

fn build_test_executor(root: &std::path::Path) -> Executor {
    Executor::new(
        Auditor::new(root.to_path_buf()).with_mode(SessionMode::Manual),
        Arc::new(None),
    )
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn enabled_skill_runs_and_records_user_action() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("skill-action-ok");
    let home = crate::test_support::temp_dir("skill-action-home");
    let skill_dir = root.join(".claude").join("skills").join("demo");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: demo\ndescription: Demo skill\n---\nbody\n",
    )
    .unwrap();
    manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();

    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_skill_action(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "/project-.claude-demo",
    )
    .await;

    assert!(handled);
    assert_eq!(session.len(), 1);
    let record = session.latest(1).unwrap();
    assert_eq!(record.calls.len(), 1);
    assert_eq!(record.calls[0].tool_name, "skill");
    assert_eq!(record.results.len(), 1);
    assert!(record.results[0].success);
    assert!(record.results[0].output.contains("invoke_skill"));
    assert!(record.results[0].audit_decisions.is_empty());

    let copied = provider.read().unwrap();
    assert!(copied.contains("[USER_ACTION kind=\"skill\" name=\"demo\"]"));
    assert!(copied.contains("[END USER_ACTION]"));
    assert!(!copied.contains("[TOOL_RESULT skill"));
    assert!(!copied.contains("<system-reminder>"));
    assert!(copied.contains("body"));

    let empty = crate::test_support::temp_dir("skill-action-empty");
    manualaid_core::skill::reload_skills_with_home(&empty, &empty).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&empty);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn disabled_skill_prints_hint_and_does_not_record() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("skill-action-disabled-root");
    let home = crate::test_support::temp_dir("skill-action-disabled-home");
    let skill_dir = home.join(".claude").join("skills").join("extra");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: extra\ndescription: Disabled global skill\n---\nbody\n",
    )
    .unwrap();
    manualaid_core::skill::reload_skills_with_home(&root, &home).unwrap();

    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_skill_action(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "/extra",
    )
    .await;

    assert!(handled);
    assert!(session.is_empty());
    assert!(_capture.text().contains("disabled"));
    assert!(provider.read().unwrap().is_empty());

    let empty = crate::test_support::temp_dir("skill-action-empty");
    manualaid_core::skill::reload_skills_with_home(&empty, &empty).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&empty);
}

#[allow(clippy::await_holding_lock)]
#[tokio::test]
async fn unknown_skill_returns_false_for_inline_fallback() {
    let _capture = crate::console::capture();
    let _locale = crate::test_support::acquire_locale_lock();
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("skill-action-unknown");
    let provider = MockClipboard::new();
    let executor = build_test_executor(&root);
    let mut config = manualaid_ws::config::Config::default();
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let handled = run_skill_action(
        &provider,
        &executor,
        &root,
        &mut config,
        &mut session,
        &mut options,
        "/does-not-exist",
    )
    .await;

    assert!(!handled);
    assert!(session.is_empty());
    assert!(provider.read().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&root);
}
