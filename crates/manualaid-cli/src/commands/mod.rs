//! Command handlers for the `manualaid-cli` executable: each subcommand's
//! `run_*` function prints the formatted output through the pager and
//! returns `Ok` or a localized error message.
//! `manualaid-cli` 可执行程序的命令处理：每个子命令的 `run_*` 函数通过
//! 分页器输出格式化结果，返回 `Ok` 或本地化错误信息。

use std::path::Path;

use crate::cli::{Cli, Command};
use crate::env::default_message;
use crate::{format_default_output, format_error_output, style};

pub mod debug;
mod dir;
mod init;
pub mod loop_cli;

pub use debug::{
    run_mask, run_mask_with_home, run_plan_edit, run_restore, run_shell_debug, run_skill,
    run_skill_with_home, run_whitelist,
};
pub use dir::{
    run_dir_clean, run_dir_clean_with_home, run_dir_clean_with_stdin, run_dir_view,
    run_dir_view_with_home,
};
pub use init::{run_init, run_init_with_home};
pub use loop_cli::run_loop;

use dir::{DirAction, run_dir};

/// Run the CLI with the current process settings and return the exit code.
/// 使用当前进程设置运行 CLI 并返回退出码。
pub fn run_main(cli: Cli) -> i32 {
    i18n::set_locale(cli.lang.as_deref().unwrap_or("en"));
    style::auto_init();
    match run(cli, None) {
        Ok(()) => 0,
        Err(error) => {
            eprint!("{}", format_error_output(&error));
            1
        }
    }
}

/// Dispatch the parsed CLI to the matching command handler.
/// 将解析后的 CLI 分发到对应的命令处理函数。
pub fn run(cli: Cli, home: Option<&Path>) -> Result<(), String> {
    match cli.command {
        None => {
            crate::console::out_print!("{}", format_default_output(&default_message()));
            run_loop(home, cli.lang, cli.mode.map(Into::into))
        }
        Some(Command::Debug { action }) => debug::run_debug(action, home),
        Some(Command::Init { project, global }) => run_init(project, global, home),
        Some(Command::Dir {
            init,
            view,
            clean: _,
            project,
            global,
            limit,
            depth,
            yes,
        }) => {
            let action = if init {
                DirAction::Init
            } else if view {
                DirAction::View
            } else {
                DirAction::Clean
            };
            run_dir(action, project, global, limit, depth, yes, home)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::test_support::{CWD_LOCK, LOCALE_LOCK, SKILL_LOCK};

    #[test]
    fn run_main_without_subcommand_starts_the_loop() {
        let _capture = crate::console::capture();
        let _cwd = CWD_LOCK.lock().unwrap();
        let _lang = LOCALE_LOCK.lock().unwrap();
        let _skills = SKILL_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        let dir = crate::test_support::temp_dir("run-main");
        std::env::set_current_dir(&dir).unwrap();
        // No command: the default branch prints the startup message and
        // enters the loop, which ends immediately on scripted stdin EOF.
        // 无子命令：默认分支打印启动消息并进入 loop，脚本 stdin 的 EOF
        // 让 loop 立即结束。
        let code = run_main(Cli {
            lang: None,
            mode: None,
            command: None,
        });
        assert_eq!(code, 0);
        std::env::set_current_dir(&original).unwrap();
        assert!(dir.join(".ManualAid").join("config.toml").is_file());
        assert!(dir.join(".ManualAid").join(".gitignore").is_file());
        assert_eq!(
            std::fs::read_to_string(dir.join(".ManualAid").join(".gitignore")).unwrap(),
            manualaid_core::manualaid_dir::GITIGNORE_CONTENT
        );
        manualaid_core::skill::reset_skills();
    }
}
