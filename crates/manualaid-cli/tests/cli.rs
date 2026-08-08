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
    let tmp = common::TempDir::new("no-args-en");
    // Run inside a temp dir so the loop's config sync does not pollute the
    // crate directory with a tracked `.ManualAid/config.toml`.
    // 在临时目录中运行，避免 loop 的配置同步向 crate 目录写入被跟踪的
    // `.ManualAid/config.toml`。
    let output = run_in(tmp.path(), tmp.path(), &[]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("ManualAid running..."));
}

#[test]
fn no_args_prints_running_message_in_chinese() {
    let tmp = common::TempDir::new("no-args-zh");
    let output = run_in(tmp.path(), tmp.path(), &["--lang", "zh-CN"]);
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

#[test]
fn init_creates_project_and_global_folders() {
    let tmp = common::TempDir::new("init-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();

    let output = run_in(&project, &home, &["init"]);
    assert!(output.status.success());
    assert!(project.join(".ManualAid").join("config.toml").is_file());
    assert!(project.join(".ManualAid").join(".gitignore").is_file());
    assert!(home.join(".ManualAid").join("config.toml").is_file());
    assert!(!home.join(".ManualAid").join(".gitignore").exists());
}

#[test]
fn init_output_includes_localized_timings() {
    let tmp = common::TempDir::new("init-timing-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();

    let output = run_in(&project, &home, &["init"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Timings"));
    assert!(text.contains("Init:"));
    assert!(text.contains("ms"));

    let output = run_in(&project, &home, &["init", "--lang", "zh-CN"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("耗时"));
    assert!(text.contains("初始化："));
}

#[test]
fn dir_view_shows_tree_and_honors_limit_and_depth() {
    let tmp = common::TempDir::new("view-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();
    fs::write(project.join(".ManualAid").join("config.toml"), "[skill]\n").unwrap();
    fs::write(project.join(".ManualAid").join(".gitignore"), "*\n").unwrap();
    for i in 0..10 {
        fs::write(project.join(".ManualAid").join(format!("f{i}.txt")), "x").unwrap();
    }

    let output = run_in(&project, &home, &["dir", "--view", "--project"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains(&format!("- {}", project.join(".ManualAid").display())));
    assert!(text.contains("config.toml"));
    assert!(text.contains(".gitignore"));
    assert!(text.contains("… 5 more files"));

    let output = run_in(
        &project,
        &home,
        &["dir", "--view", "--project", "--limit", "0"],
    );
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("f9.txt"));
    assert!(!text.contains("more files"));

    let output = run_in(
        &project,
        &home,
        &["dir", "--view", "--project", "--depth", "0"],
    );
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(!text.contains("config.toml"));
    assert!(!text.contains("├── "));
}

#[test]
fn dir_view_missing_reports_not_exists() {
    let tmp = common::TempDir::new("view-missing-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();

    let output = run_in(&project, &home, &["dir", "--view", "--project"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("does not exist"));
}

#[test]
fn dir_clean_removes_and_reports_stats() {
    let tmp = common::TempDir::new("clean-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(project.join(".ManualAid").join("logs")).unwrap();
    fs::write(project.join(".ManualAid").join("config.toml"), "[skill]\n").unwrap();
    fs::write(project.join(".ManualAid").join("data.bin"), vec![0u8; 2048]).unwrap();
    fs::write(project.join(".ManualAid").join("logs").join("a.log"), "x").unwrap();

    let output = run_in(&project, &home, &["dir", "--clean", "--project", "--yes"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Removed"));
    assert!(text.contains("3 files"));
    assert!(text.contains("2.009 KB"));
    assert!(!project.join(".ManualAid").exists());
}

#[test]
fn dir_clean_without_yes_is_rejected_when_non_terminal() {
    let tmp = common::TempDir::new("clean-reject-bin");
    let project = tmp.path().join("project");
    let home = tmp.path().join("home");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(project.join(".ManualAid")).unwrap();
    fs::write(project.join(".ManualAid").join("config.toml"), "[skill]\n").unwrap();

    let output = run_in(&project, &home, &["dir", "--clean", "--project"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Refusing to clean"));
    assert!(project.join(".ManualAid").exists());
}

#[test]
fn mask_output_includes_timings_with_char_count() {
    if !home_dir_resolvable() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let output = run(&["mask", "mail me at bob@example.com"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Timings"));
    assert!(text.contains("(26 chars)"));
}

#[test]
fn skill_output_includes_timings() {
    if !home_dir_resolvable() {
        eprintln!("skipping: home directory cannot be resolved in this environment");
        return;
    }
    let output = run(&["skill"]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Timings"));
    assert!(text.contains("Skill scan:"));
}
