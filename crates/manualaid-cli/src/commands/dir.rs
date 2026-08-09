//! //! The `dir` command: initialize, view or clean the project and/or global
//! `.ManualAid` folders, asking for confirmation before cleaning.
//! `dir` 命令：初始化、查看或清理项目/全局 `.ManualAid` 文件夹，清理前
//! 请求确认。

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use manualaid_core::manualaid_dir::clean_manualaid_dir;
use manualaid_core::timer;

use super::init::run_init;
use crate::dir_tree::{DEFAULT_VIEW_DEPTH, DEFAULT_VIEW_LIMIT, DirViewConfig, format_dir_tree};
use crate::env::{current_dir, home_dir};
use crate::{format_bytes, format_duration, format_timings, pager, t_fmt};

/// The action selected by `dir --init|--view|--clean`.
/// `dir --init|--view|--clean` 选中的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirAction {
    Init,
    View,
    Clean,
}

/// Dispatch a `dir` action to its handler.
/// 将 `dir` 动作分发到对应处理器。
pub(crate) fn run_dir(
    action: DirAction,
    project: bool,
    global: bool,
    limit: Option<i64>,
    depth: Option<i64>,
    yes: bool,
    home: Option<&Path>,
) -> Result<(), String> {
    match action {
        DirAction::Init => run_init(project, global, home),
        DirAction::View => run_dir_view(project, global, limit, depth, home),
        DirAction::Clean => run_dir_clean(project, global, yes, home),
    }
}

/// Show the file tree of the selected `.ManualAid` folders.
/// 显示选中的 `.ManualAid` 文件夹的文件树。
pub fn run_dir_view(
    project: bool,
    global: bool,
    limit: Option<i64>,
    depth: Option<i64>,
    home: Option<&Path>,
) -> Result<(), String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir()?,
    };
    run_dir_view_with_home(project, global, limit, depth, &home)
}

/// Like [`run_dir_view`] with an explicit home directory, used
/// by tests to avoid touching the real user home.
/// 同 [`run_dir_view`]，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。
pub fn run_dir_view_with_home(
    project: bool,
    global: bool,
    limit: Option<i64>,
    depth: Option<i64>,
    home: &Path,
) -> Result<(), String> {
    let project_root = current_dir()?;
    let config = DirViewConfig {
        depth: match depth {
            None => Some(DEFAULT_VIEW_DEPTH),
            Some(value) if value < 0 => None,
            Some(value) => Some(value as usize),
        },
        per_level_limit: match limit {
            None => Some(DEFAULT_VIEW_LIMIT),
            Some(value) if value <= 0 => None,
            Some(value) => Some(value as usize),
        },
    };
    let (result, elapsed) =
        timer::time(|| view_manualaid_dirs(project, global, &project_root, home, &config));
    let sections = result?;
    let output = format!(
        "{}{}",
        sections.join("\n\n"),
        format_timings(&[t_fmt(
            "cli.output.timing_dir_view",
            &[("elapsed", &format_duration(elapsed))],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Build one tree section per selected `.ManualAid` folder.
/// 为每个选中的 `.ManualAid` 文件夹构建一个树区块。
pub(crate) fn view_manualaid_dirs(
    project: bool,
    global: bool,
    project_root: &Path,
    home: &Path,
    config: &DirViewConfig,
) -> Result<Vec<String>, String> {
    let mut sections = Vec::new();
    if !global || project {
        sections.push(view_one(&project_root.join(".ManualAid"), config)?);
    }
    if !project || global {
        sections.push(view_one(&home.join(".ManualAid"), config)?);
    }
    Ok(sections)
}

/// Render one `.ManualAid` folder as a tree, or a localized "does not exist"
/// line when it is missing.
/// 将一个 `.ManualAid` 文件夹渲染为树；缺失时输出本地化的“不存在”行。
pub(crate) fn view_one(dir: &Path, config: &DirViewConfig) -> Result<String, String> {
    if !dir.exists() {
        return Ok(t_fmt(
            "cli.dir.missing",
            &[
                ("path", &dir.display().to_string()),
                ("status", &i18n::t_str("not_exists")),
            ],
        ));
    }
    format_dir_tree(dir, config)
        .map_err(|e| t_fmt("cli.error.dir_view", &[("error", &e.to_string())]))
}

/// Remove the selected `.ManualAid` folders, asking for confirmation unless
/// `--yes` is given.
/// 删除选中的 `.ManualAid` 文件夹；除非带 `--yes`，否则先请求确认。
pub fn run_dir_clean(
    project: bool,
    global: bool,
    yes: bool,
    home: Option<&Path>,
) -> Result<(), String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir()?,
    };
    run_dir_clean_with_home(project, global, yes, &home)
}

/// Like [`run_dir_clean`] with an explicit home directory,
/// used by tests to avoid touching the real user home.
/// 同 [`run_dir_clean`]，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。
pub fn run_dir_clean_with_home(
    project: bool,
    global: bool,
    yes: bool,
    home: &Path,
) -> Result<(), String> {
    run_dir_clean_with_stdin(
        project,
        global,
        yes,
        home,
        io::stdin().is_terminal(),
        &mut io::stdin().lock(),
    )
}

/// Like [`run_dir_clean_with_home`] with an
/// injectable stdin, so tests can deterministically exercise both the
/// terminal and non-terminal confirmation paths without depending on the
/// host environment.
/// 同 [`run_dir_clean_with_home`]，但 stdin 可注入，
/// 供测试在不依赖宿主环境的情况下稳定覆盖终端/非终端确认分支。
pub fn run_dir_clean_with_stdin(
    project: bool,
    global: bool,
    yes: bool,
    home: &Path,
    stdin_is_terminal: bool,
    stdin: &mut impl io::BufRead,
) -> Result<(), String> {
    let project_root = current_dir()?;
    let mut bases = Vec::new();
    if !global || project {
        bases.push(project_root.clone());
    }
    if !project || global {
        bases.push(home.to_path_buf());
    }
    let targets: Vec<PathBuf> = bases.iter().map(|base| base.join(".ManualAid")).collect();
    confirm_or_abort(&targets, yes, stdin_is_terminal, || {
        let mut answer = String::new();
        stdin.read_line(&mut answer)?;
        Ok(answer)
    })?;
    let (result, elapsed) = timer::time(|| clean_manualaid_dirs(&bases));
    let lines = result?;
    let output = format!(
        "{}{}",
        lines.join("\n"),
        format_timings(&[t_fmt(
            "cli.output.timing_dir_clean",
            &[("elapsed", &format_duration(elapsed))],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Ask for confirmation before removing any existing target. Returns `Ok`
/// when `--yes` is given, nothing exists, the user answers `y`/`Y`, or stdin
/// is not a terminal and the targets are untouched; returns an error when a
/// non-terminal stdin needs confirmation or the user aborts.
/// 删除任何存在的目标前请求确认。当带 `--yes`、目标不存在、用户回答
/// `y`/`Y` 时返回 `Ok`；stdin 非终端且需要确认、或用户中止时返回错误。
pub(crate) fn confirm_or_abort(
    targets: &[PathBuf],
    yes: bool,
    stdin_is_terminal: bool,
    mut read_answer: impl FnMut() -> io::Result<String>,
) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    let existing: Vec<&PathBuf> = targets.iter().filter(|target| target.exists()).collect();
    if existing.is_empty() {
        return Ok(());
    }
    if !stdin_is_terminal {
        return Err(t_fmt("cli.error.clean_confirm", &[]));
    }
    let paths = existing
        .iter()
        .map(|target| target.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    eprint!("{}", t_fmt("cli.dir.confirm", &[("paths", &paths)]));
    io::stderr()
        .flush()
        .map_err(|e| t_fmt("cli.error.dir_clean", &[("error", &e.to_string())]))?;
    let answer =
        read_answer().map_err(|e| t_fmt("cli.error.dir_clean", &[("error", &e.to_string())]))?;
    if !matches!(answer.trim(), "y" | "Y") {
        return Err(t_fmt("cli.dir.aborted", &[]));
    }
    Ok(())
}

/// Remove each base's `.ManualAid` directory and return one localized status
/// line per base (deleted with stats, or "does not exist").
/// 删除每个 base 的 `.ManualAid` 目录，并为每个 base 返回一行本地化状态
/// （删除统计，或“不存在”）。
pub(crate) fn clean_manualaid_dirs(bases: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    for base in bases {
        let dir = base.join(".ManualAid");
        let line = match clean_manualaid_dir(base)
            .map_err(|e| t_fmt("cli.error.dir_clean", &[("error", &e.to_string())]))?
        {
            Some(report) => t_fmt(
                "cli.dir.removed",
                &[
                    ("path", &dir.display().to_string()),
                    ("files", &report.files.to_string()),
                    ("size", &format_bytes(report.bytes)),
                ],
            ),
            None => t_fmt(
                "cli.dir.missing",
                &[
                    ("path", &dir.display().to_string()),
                    ("status", &i18n::t_str("not_exists")),
                ],
            ),
        };
        lines.push(line);
    }
    Ok(lines)
}

#[cfg(test)]
#[path = "dir_tests.rs"]
mod tests;
