//! Binary smoke tests that invoke the compiled `manualaid-cli` executable.
//! 调用编译出的 `manualaid-cli` 可执行文件的二进制冒烟测试。

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use manualaid_core::user_dir;

mod common;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_manualaid-cli"))
        .args(args)
        .output()
        .expect("run binary")
}

fn run_in(dir: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_manualaid-cli"))
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .expect("run binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("utf8 stderr")
}

fn home_dir_resolvable() -> bool {
    user_dir::home_dir().is_ok()
}

#[test]
fn no_args_prints_running_message_in_english() {
    let output = run(&[]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("ManualAid running..."));
}

#[test]
fn no_args_prints_running_message_in_chinese() {
    let output = run(&["--lang", "zh-CN"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("ManualAid正在运行..."));
}

#[test]
fn mask_prints_masked_text_and_snapshot_json() {
    if !home_dir_resolvable() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let output = run(&["mask", "mail me at bob@example.com"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("[PRV_EMAIL_"));
    assert!(text.contains("bob@example.com"));
    assert!(text.contains("{"));
    assert!(text.contains("Masked text"));
    assert!(text.contains("Snapshot JSON"));
}

#[test]
fn mask_directory_input_fails_with_localized_error() {
    if !home_dir_resolvable() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let tmp = common::TempDir::new("mask-dir");
    let output = run(&["mask", tmp.path().to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Masking failed"));
}

#[test]
fn restore_roundtrips_from_masked_file_and_snapshot_file() {
    let tmp = common::TempDir::new("restore-bin");
    let masked_path = tmp.path().join("masked.txt");
    let snapshot_path = tmp.path().join("snapshot.json");
    fs::write(&masked_path, "contact [PRV_EMAIL_1]").unwrap();
    fs::write(&snapshot_path, r#"{"[PRV_EMAIL_1]":"jane@example.com"}"#).unwrap();

    let output = run(&[
        "restore",
        masked_path.to_str().unwrap(),
        "--snapshot",
        snapshot_path.to_str().unwrap(),
    ]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("contact jane@example.com"));
    assert!(!stdout(&output).contains("Restored text"));
}

#[test]
fn restore_invalid_snapshot_fails_localized() {
    let tmp = common::TempDir::new("restore-invalid-bin");
    let snapshot_path = tmp.path().join("snapshot.json");
    fs::write(&snapshot_path, "not json").unwrap();

    let output = run(&[
        "restore",
        "[PRV_EMAIL_1]",
        "--snapshot",
        snapshot_path.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Failed to parse snapshot"));

    let output = run(&[
        "restore",
        "[PRV_EMAIL_1]",
        "--snapshot",
        snapshot_path.to_str().unwrap(),
        "--lang",
        "zh-CN",
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("快照解析失败"));
}

#[test]
fn skill_flags_filter_global_and_project_scopes() {
    if !home_dir_resolvable() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let tmp = common::TempDir::new("skill-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();
    common::write_skill(
        &project,
        ".claude",
        "projskill",
        Some("projskill"),
        "project description",
    );
    common::write_skill(
        &home,
        ".codex",
        "globskill",
        Some("globskill"),
        "global description",
    );

    let both = run_in(&project, &home, &["skill"]);
    assert!(both.status.success());
    assert!(stdout(&both).contains("projskill"));
    assert!(stdout(&both).contains("globskill"));

    let global_only = run_in(&project, &home, &["skill", "--global"]);
    assert!(global_only.status.success());
    assert!(stdout(&global_only).contains("globskill"));
    assert!(!stdout(&global_only).contains("projskill"));

    let project_only = run_in(&project, &home, &["skill", "--project"]);
    assert!(project_only.status.success());
    assert!(stdout(&project_only).contains("projskill"));
    assert!(!stdout(&project_only).contains("globskill"));

    let both_flags = run_in(&project, &home, &["skill", "--global", "--project"]);
    assert!(both_flags.status.success());
    assert!(stdout(&both_flags).contains("projskill"));
    assert!(stdout(&both_flags).contains("globskill"));
}
