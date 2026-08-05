//! The `mask` command: mask sensitive data in text or a file and print the
//! masked text plus the pretty snapshot JSON.
//! `mask` 命令：掩码文本或文件中的敏感数据，输出掩码文本与 pretty 快照 JSON。

use std::path::Path;

use manualaid_core::privacy::{PrivacyMaskExtension, PrivacyMasker};
use manualaid_core::timer;

use crate::env::{current_dir, home_dir};
use crate::{format_duration, format_mask_output, format_timings, mask_with_chars, pager, t_fmt};

/// Mask the input and print the masked text plus the pretty snapshot JSON.
/// 掩码输入并输出掩码文本与 pretty 快照 JSON。
pub fn run_mask(input: &str, home: Option<&Path>) -> Result<(), String> {
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir()?,
    };
    run_mask_with_home(input, &home)
}

/// Like [`run_mask`](run_mask) with an explicit home directory, used by
/// tests to avoid touching the real user home.
/// 同 [`run_mask`](run_mask)，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。
pub fn run_mask_with_home(input: &str, home: &Path) -> Result<(), String> {
    let extensions = PrivacyMaskExtension::load_with_home(&current_dir()?, home)
        .map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let masker = PrivacyMasker::from_extensions(&extensions)
        .map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let (result, elapsed) = timer::time(|| mask_with_chars(&masker, input));
    let (masked, snapshot, chars) =
        result.map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))?;
    let output = format!(
        "{}{}",
        format_mask_output(&masked, &json),
        format_timings(&[t_fmt(
            "cli.output.timing_mask",
            &[
                ("elapsed", &format_duration(elapsed)),
                ("chars", &chars.to_string()),
            ],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}
