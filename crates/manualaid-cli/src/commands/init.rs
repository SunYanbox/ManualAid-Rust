//! The `init` command: initialize the project and/or global `.ManualAid`
//! folders.
//! `init` 命令：初始化项目/全局 `.ManualAid` 文件夹。

use std::path::Path;

use manualaid_core::manualaid_dir::{ensure_global_manualaid_dir, ensure_project_manualaid_dir};
use manualaid_core::timer;

use crate::env::{current_dir, home_dir};
use crate::{format_duration, format_timings, pager, t_fmt};

/// Initialize the project and/or global `.ManualAid` folders.
/// 初始化项目/全局 `.ManualAid` 文件夹。
pub fn run_init(project: bool, global: bool, home: Option<&Path>) -> Result<(), String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir()?,
    };
    run_init_with_home(project, global, &home)
}

/// Like [`run_init`] with an explicit home directory, used by tests
/// to avoid touching the real user home.
/// 同 [`run_init`]，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。
pub fn run_init_with_home(project: bool, global: bool, home: &Path) -> Result<(), String> {
    let project_root = current_dir()?;
    let (result, elapsed) =
        timer::time(|| init_manualaid_dirs(project, global, &project_root, home));
    let lines = result?;
    let output = format!(
        "{}{}",
        lines.join("\n"),
        format_timings(&[t_fmt(
            "cli.output.timing_init",
            &[("elapsed", &format_duration(elapsed))],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Create the selected `.ManualAid` folders and return one localized status
/// line per created folder.
/// 创建选中的 `.ManualAid` 文件夹，并为每个文件夹返回一行本地化状态。
pub(crate) fn init_manualaid_dirs(
    project: bool,
    global: bool,
    project_root: &Path,
    home: &Path,
) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    if !global || project {
        ensure_project_manualaid_dir(project_root)
            .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
        lines.push(t_fmt(
            "cli.dir.initialized",
            &[(
                "path",
                &project_root.join(".ManualAid").display().to_string(),
            )],
        ));
    }
    if !project || global {
        ensure_global_manualaid_dir(home)
            .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
        lines.push(t_fmt(
            "cli.dir.initialized",
            &[("path", &home.join(".ManualAid").display().to_string())],
        ));
    }
    Ok(lines)
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
