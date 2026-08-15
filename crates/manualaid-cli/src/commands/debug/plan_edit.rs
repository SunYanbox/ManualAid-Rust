//! The `debug plan_edit` command: pre-verify an edit `old_string` against a
//! file through the real core validation path, without modifying the file.
//! `debug plan_edit` 命令：通过 core 的真实校验路径预检 edit 的
//! `old_string` 是否匹配文件，不修改文件。

use std::time::Duration;

use indexmap::IndexMap;
use manualaid_core::timer;
use manualaid_core::tools::edit::plan_edit;
use serde_json::Value;

use super::resolve_arg;
use crate::{format_duration, format_timings, t_fmt};

/// Pre-verify one edit `old_string` against `path` and print the occurrence
/// count plus the full searched text, followed by a timing line (scan
/// duration and scanned character count) on success or failure.
/// 预检 `old_string` 是否匹配 `path`，输出出现次数与完整的搜索原文本，
/// 随后无论成功或失败都输出耗时行（扫描时长与扫描字符数）。
///
/// # Description
/// `replace_all=true` and an empty `new_string` are injected internally, so
/// multiple matches report a `count` instead of an ambiguity error and the
/// two strings are guaranteed different. The target file is never modified.
/// On the error path the scanned character count is re-derived from the file
/// when readable (0 otherwise), so the timing line stays informative.
/// # 描述
/// 内部固定注入 `replace_all=true` 与空 `new_string`，多处匹配时报告
/// `count` 而非歧义错误，且两个字符串必然不同。目标文件不会被修改。
/// 失败路径在文件可读时重新读取以推导扫描字符数（否则按 0 计），保证
/// 耗时行仍有参考价值。
pub async fn run_plan_edit(path: &str, old_string: &str) -> Result<(), String> {
    let old_string = resolve_arg(old_string)?;
    if old_string.is_empty() {
        return Err(t_fmt("cli.debug.plan_old_empty", &[]));
    }
    let mut params = IndexMap::new();
    params.insert("file_path".to_string(), Value::String(path.to_string()));
    params.insert("old_string".to_string(), Value::String(old_string.clone()));
    params.insert("new_string".to_string(), Value::String(String::new()));
    params.insert("replace_all".to_string(), Value::Bool(true));
    let (result, elapsed) = timer::time_async(plan_edit(&params)).await;
    match result {
        Ok(plan) => {
            crate::console::out_println!(
                "{}",
                t_fmt(
                    "cli.debug.plan_found",
                    &[
                        ("count", &plan.count.to_string()),
                        ("path", &plan.file_path),
                    ]
                )
            );
            crate::console::out_println!("{}", t_fmt("cli.debug.plan_original", &[]));
            crate::console::out_println!("{}", plan.old_string);
            print_timing(elapsed, plan.content.chars().count());
            Ok(())
        }
        Err(message) => {
            let chars = std::fs::read_to_string(path)
                .map(|content| content.chars().count())
                .unwrap_or(0);
            print_timing(elapsed, chars);
            Err(message)
        }
    }
}

/// Print the plan_edit timing line through the console sink.
/// 通过控制台出口输出 plan_edit 的耗时行。
fn print_timing(elapsed: Duration, chars: usize) {
    let timings = format_timings(&[t_fmt(
        "cli.output.timing_plan_edit",
        &[
            ("elapsed", &format_duration(elapsed)),
            ("chars", &chars.to_string()),
        ],
    )]);
    crate::console::out_print!("{timings}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{LOCALE_LOCK, temp_dir};

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn reports_count_and_full_original_text() {
        let _capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("plan-edit");
        let file = dir.join("target.txt");
        std::fs::write(&file, "alpha beta alpha gamma").unwrap();
        let path = file.display().to_string();
        assert!(run_plan_edit(&path, "alpha").await.is_ok());
        let text = _capture.text();
        assert!(text.contains("Found 2 occurrence(s)"));
        assert!(text.contains("Searched text"));
        assert!(text.contains("alpha"));
        assert!(text.contains("Plan edit:"), "timing missing in: {text}");
        assert!(text.contains("(22 chars)"), "char count missing in: {text}");
        // The file must stay untouched.
        // 文件必须保持原样。
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "alpha beta alpha gamma"
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn errors_when_target_file_is_missing() {
        let _capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("plan-edit-missing");
        let missing = dir.join("missing.txt");
        let err = run_plan_edit(&missing.display().to_string(), "alpha")
            .await
            .unwrap_err();
        assert!(!err.is_empty());
        // The failure path still reports the timing; an unreadable file
        // counts as zero scanned characters.
        // 失败路径仍输出耗时；文件不可读时按零个扫描字符计。
        let text = _capture.text();
        assert!(text.contains("Plan edit:"), "timing missing in: {text}");
        assert!(text.contains("(0 chars)"), "char count missing in: {text}");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn errors_when_old_string_is_absent() {
        let _capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("plan-edit-absent");
        let file = dir.join("target.txt");
        std::fs::write(&file, "only beta here").unwrap();
        let err = run_plan_edit(&file.display().to_string(), "alpha")
            .await
            .unwrap_err();
        assert!(err.contains("not found"));
        // The scanned file content is still reported on the failure path.
        // 失败路径仍报告实际扫描的文件字符数。
        let text = _capture.text();
        assert!(text.contains("Plan edit:"), "timing missing in: {text}");
        assert!(text.contains("(14 chars)"), "char count missing in: {text}");
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn rejects_empty_old_string() {
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("plan-edit-empty");
        let file = dir.join("target.txt");
        std::fs::write(&file, "content").unwrap();
        let err = run_plan_edit(&file.display().to_string(), "")
            .await
            .unwrap_err();
        assert!(err.contains("must not be empty"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn resolves_at_prefixed_old_string_from_file() {
        let _capture = crate::console::capture();
        let _lang = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("plan-edit-at");
        let file = dir.join("target.txt");
        std::fs::write(&file, "needle needle").unwrap();
        let needle = dir.join("needle.txt");
        std::fs::write(&needle, "needle").unwrap();
        let path = file.display().to_string();
        let old_arg = format!("@{}", needle.display());
        assert!(run_plan_edit(&path, &old_arg).await.is_ok());
        assert!(_capture.text().contains("Found 2 occurrence(s)"));
        assert!(_capture.text().contains("Plan edit:"));
    }
}
