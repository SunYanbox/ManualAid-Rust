//! Unit tests for the non-interactive `copy` subcommand.
//! 非交互式 `copy` 子命令的单元测试。

use std::path::Path;

use clap::Parser;
use manualaid_core::clipboard::MockClipboard;

use super::*;
use crate::cli::{Cli, ContextFilesSpec, CopyKind};

fn write_context_file(root: &Path, name: &str) {
    std::fs::write(root.join(name), format!("# {name}\n")).unwrap();
}

fn run_with_mock(
    root: &Path,
    kind: CopyKind,
    lang: Option<String>,
    spec: ContextFilesSpec,
    provider: &MockClipboard,
) -> Result<(), String> {
    let home = crate::test_support::temp_dir("copy-home");
    run_copy_at_with_provider(root, &home, kind, lang, spec, provider)
}

#[test]
fn clap_parses_all_copy_kind_names_and_aliases() {
    let cases = [
        ("system-prompt", CopyKind::SystemPrompt),
        ("system", CopyKind::SystemPrompt),
        ("compressed-session", CopyKind::CompressedSession),
        ("compress", CopyKind::CompressedSession),
        ("intent-rule", CopyKind::IntentRule),
        ("intent", CopyKind::IntentRule),
        ("tool-format", CopyKind::ToolFormat),
        ("tool-fmt", CopyKind::ToolFormat),
        ("enabled-tools", CopyKind::EnabledTools),
        ("context", CopyKind::Context),
        ("ctx", CopyKind::Context),
        ("line-ending-rule", CopyKind::LineEndingRule),
        ("line-ending", CopyKind::LineEndingRule),
        ("plan-mode-rule", CopyKind::PlanModeRule),
        ("plan", CopyKind::PlanModeRule),
        ("switch-mode-rule", CopyKind::SwitchModeRule),
        ("build", CopyKind::SwitchModeRule),
        ("task-planning-rule", CopyKind::TaskPlanningRule),
        ("task", CopyKind::TaskPlanningRule),
    ];
    for (input, expected) in cases {
        let cli = Cli::try_parse_from(["manualaid-cli", "copy", input]).unwrap();
        match cli.command {
            Some(crate::cli::Command::Copy { kind, .. }) => assert_eq!(kind, expected),
            other => panic!("unexpected command for {input}: {other:?}"),
        }
    }
}

#[test]
fn clap_rejects_unknown_copy_kind_and_enabled_tools_alias() {
    assert!(Cli::try_parse_from(["manualaid-cli", "copy", "tools"]).is_err());
    assert!(Cli::try_parse_from(["manualaid-cli", "copy", "bogus"]).is_err());
}

#[test]
fn clap_parses_context_files_spec_defaults_to_first() {
    let cli = Cli::try_parse_from(["manualaid-cli", "copy", "system"]).unwrap();
    match cli.command {
        Some(crate::cli::Command::Copy {
            context_files: Some(ContextFilesSpec::First),
            ..
        }) => {}
        other => panic!("unexpected context files default: {other:?}"),
    }
}

#[test]
fn resolve_context_files_follows_first_all_none() {
    let root = crate::test_support::temp_dir("resolve-context");
    assert!(resolve_context_files(ContextFilesSpec::First, &root).is_empty());
    assert!(resolve_context_files(ContextFilesSpec::All, &root).is_empty());
    assert!(resolve_context_files(ContextFilesSpec::None, &root).is_empty());

    write_context_file(&root, "AGENTS.md");
    let first = resolve_context_files(ContextFilesSpec::First, &root);
    assert_eq!(first.len(), 1);
    assert!(first[0].ends_with("AGENTS.md"));
    assert_eq!(resolve_context_files(ContextFilesSpec::All, &root).len(), 1);
    assert!(resolve_context_files(ContextFilesSpec::None, &root).is_empty());

    write_context_file(&root, "CLAUDE.md");
    assert_eq!(
        resolve_context_files(ContextFilesSpec::First, &root).len(),
        1
    );
    assert_eq!(resolve_context_files(ContextFilesSpec::All, &root).len(), 2);
}

#[test]
fn system_prompt_copy_respects_context_files_none_and_first() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-system");
    write_context_file(&root, "AGENTS.md");
    let mock = MockClipboard::new();

    run_with_mock(
        &root,
        CopyKind::SystemPrompt,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &mock,
    )
    .unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.contains("<system_prompt>"));
    assert!(!copied.contains("Instructions from: AGENTS.md"));
    assert!(!copied.ends_with("</system-reminder>"));

    run_with_mock(
        &root,
        CopyKind::SystemPrompt,
        Some("en".to_string()),
        ContextFilesSpec::First,
        &mock,
    )
    .unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.contains("<system_prompt>"));
    assert!(copied.contains("<system-reminder>"));
    assert!(copied.contains("Instructions from: AGENTS.md"));
}

#[test]
fn context_copy_renders_reminder_block() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-context");
    write_context_file(&root, "AGENTS.md");
    let mock = MockClipboard::new();

    run_with_mock(
        &root,
        CopyKind::Context,
        Some("en".to_string()),
        ContextFilesSpec::First,
        &mock,
    )
    .unwrap();
    let copied = mock.read().unwrap();
    assert!(copied.starts_with("<system-reminder>\n"));
    assert!(copied.contains("Instructions from: AGENTS.md"));
    assert!(copied.ends_with("</system-reminder>"));

    // `none` prints the no-files message and leaves the clipboard unchanged.
    run_with_mock(
        &root,
        CopyKind::Context,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &mock,
    )
    .unwrap();
    assert_eq!(mock.read().unwrap(), copied);
}

#[test]
fn static_copy_kinds_produce_expected_reminder_content() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-static");

    let cases = [
        (CopyKind::CompressedSession, "<system-reminder>"),
        (CopyKind::IntentRule, ""),
        (CopyKind::ToolFormat, ""),
        (CopyKind::EnabledTools, ""),
        (CopyKind::LineEndingRule, ""),
        (CopyKind::PlanModeRule, ""),
        (CopyKind::SwitchModeRule, ""),
        (CopyKind::TaskPlanningRule, ""),
    ];
    for (kind, expected_marker) in cases {
        let mock = MockClipboard::new();
        run_with_mock(
            &root,
            kind,
            Some("en".to_string()),
            ContextFilesSpec::None,
            &mock,
        )
        .unwrap();
        let copied = mock.read().unwrap();
        assert!(
            !copied.is_empty(),
            "{kind:?} should copy a non-empty prompt"
        );
        if !expected_marker.is_empty() {
            assert!(copied.contains(expected_marker), "{kind:?}");
        }
    }
}

#[test]
fn run_copy_with_provider_uses_process_directories() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _cwd = crate::test_support::CWD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-run-with-provider");
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir(&root).unwrap();
    let mock = MockClipboard::new();

    let result = run_copy_with_provider(
        CopyKind::IntentRule,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &mock,
    );

    std::env::set_current_dir(&original).unwrap();
    result.unwrap();
    let copied = mock.read().unwrap();
    assert_eq!(copied, i18n::t_str("prompt.system.intent-output-rule"));
}

#[test]
fn copy_reports_config_validation_issues() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-config-issue");
    let manualaid_dir = root.join(".ManualAid");
    std::fs::create_dir_all(&manualaid_dir).unwrap();
    std::fs::write(
        manualaid_dir.join("config.toml"),
        "[global]\nlang = \"xx\"\n",
    )
    .unwrap();
    let mock = MockClipboard::new();
    let home = crate::test_support::temp_dir("copy-config-issue-home");

    run_copy_at_with_provider(
        &root,
        &home,
        CopyKind::IntentRule,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &mock,
    )
    .unwrap();

    let output = _capture.text();
    assert!(!output.is_empty(), "config issue warning should be printed");
    assert!(
        output.contains("lang"),
        "warning should mention the lang key"
    );
}

#[test]
fn copy_write_failure_propagates_error() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i18n::set_locale("en");
    let root = crate::test_support::temp_dir("copy-write-fail");
    let mock = MockClipboard::new();
    mock.set_write_error("mock failure");

    let result = run_with_mock(
        &root,
        CopyKind::IntentRule,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &mock,
    );
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "mock failure");
}

#[test]
fn copy_locale_flag_changes_clipboard_text() {
    let _capture = crate::console::capture();
    let _lang = crate::test_support::acquire_locale_lock();
    let _skills = crate::test_support::SKILL_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = crate::test_support::temp_dir("copy-locale");

    let en_mock = MockClipboard::new();
    run_with_mock(
        &root,
        CopyKind::TaskPlanningRule,
        Some("en".to_string()),
        ContextFilesSpec::None,
        &en_mock,
    )
    .unwrap();
    let en_text = en_mock.read().unwrap();

    let zh_mock = MockClipboard::new();
    run_with_mock(
        &root,
        CopyKind::TaskPlanningRule,
        Some("zh-CN".to_string()),
        ContextFilesSpec::None,
        &zh_mock,
    )
    .unwrap();
    let zh_text = zh_mock.read().unwrap();

    assert_ne!(en_text, zh_text);
}
