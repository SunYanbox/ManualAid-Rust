//! Command handlers for the `manualaid-cli` executable: each subcommand's
//! `run_*` function prints the formatted output through the pager and
//! returns `Ok` or a localized error message.
//! `manualaid-cli` 可执行程序的命令处理：每个子命令的 `run_*` 函数通过
//! 分页器输出格式化结果，返回 `Ok` 或本地化错误信息。

use std::path::Path;

use crate::cli::{Cli, Command};
use crate::env::default_message;
use crate::{format_default_output, format_error_output, style};

mod dir;
mod init;
pub mod loop_cli;
mod mask;
mod restore;
mod skill;

pub use loop_cli::run_loop;
pub use dir::{
    run_dir_clean, run_dir_clean_with_home, run_dir_clean_with_stdin, run_dir_view,
    run_dir_view_with_home,
};
pub use init::{run_init, run_init_with_home};
pub use mask::{run_mask, run_mask_with_home};
pub use restore::run_restore;
pub use skill::{run_skill, run_skill_with_home};

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
            print!("{}", format_default_output(&default_message()));
            run_loop(home, cli.lang)
        }
        Some(Command::Mask { input }) => run_mask(&input, home),
        Some(Command::Restore { input, snapshot }) => run_restore(&input, &snapshot),
        Some(Command::Skill { global, project }) => run_skill(global, project, home),
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
