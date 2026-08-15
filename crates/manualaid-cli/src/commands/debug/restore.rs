//! The `restore` command: restore the original text from masked input plus
//! a snapshot JSON file.
//! `restore` 命令：根据掩码输入与快照 JSON 文件还原原文。

use std::path::Path;

use manualaid_core::error::CoreError;
use manualaid_core::timer;

use crate::{
    format_duration, format_restore_output, format_timings, pager, restore_with_chars, t_fmt,
};

/// Restore the original text from the masked input and snapshot file.
/// 根据掩码输入与快照文件还原原文。
pub fn run_restore(input: &str, snapshot: &Path) -> Result<(), String> {
    let (result, elapsed) = timer::time(|| restore_with_chars(input, snapshot));
    let (original, chars) = result.map_err(|e| {
        let key = match &e {
            CoreError::Parse(_) => "cli.error.snapshot_parse",
            CoreError::InvalidPath(_) => "cli.error.input_read",
            _ => "cli.error.snapshot_read",
        };
        t_fmt(key, &[("error", &e.to_string())])
    })?;
    let output = format!(
        "{}{}",
        format_restore_output(&original),
        format_timings(&[t_fmt(
            "cli.output.timing_restore",
            &[
                ("elapsed", &format_duration(elapsed)),
                ("chars", &chars.to_string()),
            ],
        )])
    );
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}
