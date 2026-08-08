//! Integration tests for the public clap CLI definitions.
//! clap CLI 公共定义的集成测试。

use std::path::Path;

use clap::Parser;

use manualaid_cli::cli::{Cli, Command, ModeArg};

fn parse(args: &[&str]) -> Cli {
    Cli::try_parse_from(args).expect("args should parse")
}

#[test]
fn parses_no_args() {
    let cli = parse(&["manualaid-cli"]);
    assert!(cli.command.is_none());
    assert!(cli.lang.is_none());
    assert!(cli.mode.is_none());
}

#[test]
fn parses_lang_flag() {
    let cli = parse(&["manualaid-cli", "--lang", "zh-CN"]);
    assert_eq!(cli.lang.as_deref(), Some("zh-CN"));
}

#[test]
fn parses_mode_flag() {
    let cli = parse(&["manualaid-cli", "--mode", "accept-edit"]);
    assert!(matches!(cli.mode, Some(ModeArg::AcceptEdit)));
    let cli = parse(&["manualaid-cli", "--mode", "manual"]);
    assert!(matches!(cli.mode, Some(ModeArg::Manual)));
}

#[test]
fn rejects_invalid_mode_value() {
    assert!(Cli::try_parse_from(["manualaid-cli", "--mode", "bogus"]).is_err());
}

#[test]
fn mode_arg_maps_to_session_mode() {
    use manualaid_core::audit::SessionMode;
    assert_eq!(SessionMode::from(ModeArg::Manual), SessionMode::Manual);
    assert_eq!(
        SessionMode::from(ModeArg::AcceptEdit),
        SessionMode::AcceptEdit
    );
}

#[test]
fn parses_mask_command() {
    let cli = parse(&["manualaid-cli", "mask", "hello"]);
    assert!(matches!(cli.command, Some(Command::Mask { input }) if input == "hello"));
}

#[test]
fn parses_restore_command() {
    let cli = parse(&["manualaid-cli", "restore", "text", "--snapshot", "s.json"]);
    assert!(matches!(
        cli.command,
        Some(Command::Restore { input, snapshot })
            if input == "text" && snapshot == Path::new("s.json")
    ));
}

#[test]
fn parses_skill_flags() {
    let cli = parse(&["manualaid-cli", "skill", "--global", "--project"]);
    assert!(matches!(
        cli.command,
        Some(Command::Skill {
            global: true,
            project: true
        })
    ));
}

#[test]
fn parses_init_command() {
    let cli = parse(&["manualaid-cli", "init", "--project"]);
    assert!(matches!(
        cli.command,
        Some(Command::Init {
            project: true,
            global: false
        })
    ));
}

#[test]
fn parses_dir_view_flags() {
    let cli = parse(&[
        "manualaid-cli",
        "dir",
        "--view",
        "--project",
        "--limit",
        "0",
        "--depth",
        "-1",
    ]);
    assert!(matches!(
        cli.command,
        Some(Command::Dir {
            view: true,
            project: true,
            limit: Some(0),
            depth: Some(-1),
            ..
        })
    ));
}

#[test]
fn parses_dir_clean_flags() {
    let cli = parse(&["manualaid-cli", "dir", "--clean", "--global", "--yes"]);
    assert!(matches!(
        cli.command,
        Some(Command::Dir {
            clean: true,
            global: true,
            yes: true,
            ..
        })
    ));
}

#[test]
fn parses_dir_init_action() {
    let cli = parse(&["manualaid-cli", "dir", "--init"]);
    assert!(matches!(cli.command, Some(Command::Dir { init: true, .. })));
}

#[test]
fn dir_requires_an_action() {
    assert!(Cli::try_parse_from(["manualaid-cli", "dir"]).is_err());
}

#[test]
fn dir_actions_are_mutually_exclusive() {
    assert!(Cli::try_parse_from(["manualaid-cli", "dir", "--view", "--clean"]).is_err());
}
