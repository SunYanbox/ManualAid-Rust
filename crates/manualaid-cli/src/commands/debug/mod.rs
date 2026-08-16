//! The `debug` command group: tool-layer diagnostics (`plan_edit`, `shell`)
//! plus the migrated `mask`/`restore`/`skill` helpers.
//! `debug` 命令组：工具层诊断（`plan_edit`、`shell`）以及迁移而来的
//! `mask`/`restore`/`skill` 辅助。

use std::path::Path;

use crate::cli::DebugAction;
use crate::t_fmt;

mod mask;
mod plan_edit;
mod restore;
pub mod shell;
mod skill;
mod whitelist;

pub use mask::{run_mask, run_mask_with_home};
pub use plan_edit::run_plan_edit;
pub use restore::run_restore;
pub use shell::run_shell_debug;
pub use skill::{run_skill, run_skill_with_home};
pub use whitelist::run_whitelist;

/// Dispatch a `debug` action to its handler; async handlers run on a fresh
/// Tokio runtime so the caller stays synchronous.
/// 将 `debug` 动作分发到对应处理器；异步处理器在新建的 Tokio runtime 上
/// 运行，调用方保持同步。
pub fn run_debug(action: DebugAction, home: Option<&Path>) -> Result<(), String> {
    match action {
        DebugAction::PlanEdit { path, old_string } => run_async(run_plan_edit(&path, &old_string)),
        DebugAction::Shell { command, time_out } => {
            run_async(run_shell_debug(&command, time_out.as_deref()))
        }
        DebugAction::Mask { input } => run_mask(&input, home),
        DebugAction::Restore { input, snapshot } => run_restore(&input, &snapshot),
        DebugAction::Skill { global, project } => run_skill(global, project, home),
        DebugAction::Whitelist { project } => run_whitelist(home, project.as_deref()),
    }
}

/// Resolve one content argument: a value starting with `@` is treated as a
/// file path whose whole content becomes the argument (trimmed by the
/// caller), anything else is used literally. Path-style arguments never go
/// through this resolution.
/// 解析内容型参数：以 `@` 开头的值把剩余部分当作文件路径，整个文件内容
/// 作为参数值（是否去除首尾空白由调用方决定）；其余值按字面使用。
/// 路径型参数不经过本解析。
fn resolve_arg(value: &str) -> Result<String, String> {
    match value.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| {
            t_fmt(
                "cli.debug.arg_read",
                &[("path", path), ("error", &e.to_string())],
            )
        }),
        None => Ok(value.to_string()),
    }
}

/// Run an async command handler on a fresh multi-thread Tokio runtime.
/// 在新建的多线程 Tokio runtime 上运行异步命令处理器。
fn run_async<F>(future: F) -> Result<(), String>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build runtime: {e}"))?;
    runtime.block_on(future)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn resolve_arg_keeps_literal_values() {
        assert_eq!(resolve_arg("hello").unwrap(), "hello");
        assert_eq!(resolve_arg("").unwrap(), "");
        assert_eq!(resolve_arg("a@b").unwrap(), "a@b");
    }

    #[test]
    fn resolve_arg_reads_at_file_content() {
        let dir = temp_dir("resolve-arg");
        let file = dir.join("needle.txt");
        std::fs::write(&file, "first line\nsecond line").unwrap();
        let resolved = resolve_arg(&format!("@{}", file.display())).unwrap();
        assert_eq!(resolved, "first line\nsecond line");
    }

    #[test]
    fn resolve_arg_errors_on_missing_file() {
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("resolve-arg-missing");
        let missing = dir.join("missing.txt");
        let err = resolve_arg(&format!("@{}", missing.display())).unwrap_err();
        assert!(err.contains("Failed to read argument file"));
    }

    #[test]
    fn run_debug_dispatches_whitelist() {
        use crate::cli::DebugAction;
        let _lang = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let dir = temp_dir("debug-whitelist-dispatch");
        std::fs::create_dir_all(dir.join(".ManualAid")).unwrap();
        let action = DebugAction::Whitelist {
            project: Some(dir.clone()),
        };
        let result = run_debug(action, Some(&dir));
        assert!(result.is_ok(), "unexpected error: {result:?}");
    }
}
