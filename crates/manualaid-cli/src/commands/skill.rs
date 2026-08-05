//! # Description
//! The `skill` command: scan, filter and print skills, honoring the
//! `--global`/`--project` flags.
//! # 描述
//! `skill` 命令：扫描、过滤并输出技能，遵循 `--global`/`--project` 旗标。

use std::path::Path;

use manualaid_core::skill::{all_skills, reload_skills_with_home};
use manualaid_core::timer;

use crate::env::{current_dir, home_dir};
use crate::{
    SkillScope, filter_skills, format_duration, format_skill_output, format_timings, pager, t_fmt,
};

/// Scan, filter and print skills, honoring the --global/--project flags.
/// 扫描、过滤并输出技能，遵循 --global/--project 旗标。
pub fn run_skill(global: bool, project: bool, home: Option<&Path>) -> Result<(), String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir()?,
    };
    run_skill_with_home(global, project, &home)
}

/// Like [`run_skill`](run_skill) with an explicit home directory, used by
/// tests to avoid touching the real user home.
/// 同 [`run_skill`](run_skill)，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。
pub fn run_skill_with_home(global: bool, project: bool, home: &Path) -> Result<(), String> {
    let dir = current_dir()?;
    let (scan_result, elapsed) = timer::time(|| reload_skills_with_home(&dir, home));
    scan_result.map_err(|e| t_fmt("cli.error.skill_scan", &[("error", &e.to_string())]))?;
    let skills = all_skills();
    let scope = match (global, project) {
        (true, true) => SkillScope::All,
        (true, false) => SkillScope::Global,
        (false, true) => SkillScope::Project,
        (false, false) => SkillScope::All,
    };
    let filtered = filter_skills(skills, scope);
    let output = format!(
        "{}{}",
        format_skill_output(&filtered),
        format_timings(&[t_fmt(
            "cli.output.timing_skill_scan",
            &[("elapsed", &format_duration(elapsed))],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}
