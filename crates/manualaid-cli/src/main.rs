use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use manualaid_core::error::CoreError;
use manualaid_core::privacy::{PrivacyMaskExtension, PrivacyMasker};
use manualaid_core::skill::{all_skills, reload_skills_with_home};
use manualaid_core::user_dir;

use manualaid_cli::pager;
use manualaid_cli::{
    SkillScope, filter_skills, format_default_output, format_error_output, format_mask_output,
    format_restore_output, format_skill_output, mask, restore, style, t_fmt,
};

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

#[derive(Parser, Debug)]
#[command(
    name = "manualaid-cli",
    version,
    about = "ManualAid command line interface"
)]
struct Cli {
    /// Interface language code: en or zh-CN
    /// 界面语言代码：en 或 zh-CN
    #[arg(short, long, global = true, default_value = "en")]
    lang: String,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Mask sensitive data in text or a file, then print the masked text and
    /// a serializable snapshot (JSON).
    /// 掩码文本或文件中的敏感数据，输出掩码文本与可序列化快照（JSON）。
    Mask {
        /// Text or path to a file
        /// 文本或文件路径
        input: String,
    },
    /// Restore the original text from masked text plus a snapshot JSON file.
    /// 根据掩码文本与快照 JSON 文件还原原文。
    Restore {
        /// Masked text or path to a file containing it
        /// 掩码文本或包含掩码文本的文件路径
        input: String,
        /// Path to the snapshot JSON file
        /// 快照 JSON 文件路径
        #[arg(long)]
        snapshot: PathBuf,
    },
    /// List scanned SKILLs; --global/--project filter the scope.
    /// 列出扫描到的 SKILL；--global/--project 过滤范围。
    Skill {
        /// Show global skills only
        /// 仅显示全局技能
        #[arg(long)]
        global: bool,
        /// Show project skills only
        /// 仅显示项目技能
        #[arg(long)]
        project: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    std::process::exit(run_main(cli));
}

/// Run the CLI with the current process settings and return the exit code.
/// 使用当前进程设置运行 CLI 并返回退出码。
fn run_main(cli: Cli) -> i32 {
    i18n::set_locale(&cli.lang);
    style::auto_init();
    match run(cli, None) {
        Ok(()) => 0,
        Err(error) => {
            eprint!("{}", format_error_output(&error));
            1
        }
    }
}

/// # Description
/// Dispatch the parsed CLI to the matching command handler.
/// # 描述
/// 将解析后的 CLI 分发到对应的命令处理函数。
fn run(cli: Cli, home: Option<&Path>) -> Result<(), String> {
    match cli.command {
        None => {
            print!("{}", format_default_output(&default_message()));
            Ok(())
        }
        Some(Command::Mask { input }) => run_mask(&input, home),
        Some(Command::Restore { input, snapshot }) => run_restore(&input, &snapshot),
        Some(Command::Skill { global, project }) => run_skill(global, project, home),
    }
}

/// The default startup message.
/// 默认启动消息。
fn default_message() -> String {
    i18n::t_str("manual-aid-running")
}

/// The current working directory, or a localized error message.
/// 当前工作目录，失败时返回本地化错误信息。
fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))
}

/// The user home directory, or a localized error message.
/// 用户主目录，失败时返回本地化错误信息。
fn home_dir() -> Result<PathBuf, String> {
    user_dir::home_dir().map_err(|e| t_fmt("cli.error.home", &[("error", &e.to_string())]))
}

/// Mask the input and print the masked text plus the pretty snapshot JSON.
/// 掩码输入并输出掩码文本与 pretty 快照 JSON。
fn run_mask(input: &str, home: Option<&Path>) -> Result<(), String> {
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
fn run_mask_with_home(input: &str, home: &Path) -> Result<(), String> {
    let extensions = PrivacyMaskExtension::load_with_home(&current_dir()?, home)
        .map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let masker = PrivacyMasker::from_extensions(&extensions)
        .map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let (masked, snapshot) =
        mask(&masker, input).map_err(|e| t_fmt("cli.error.mask", &[("error", &e.to_string())]))?;
    let json = serde_json::to_string_pretty(&snapshot)
        .map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))?;
    pager::print_paged(&format_mask_output(&masked, &json))
        .map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Restore the original text from the masked input and snapshot file.
/// 根据掩码输入与快照文件还原原文。
fn run_restore(input: &str, snapshot: &Path) -> Result<(), String> {
    let original = restore(input, snapshot).map_err(|e| {
        let key = match &e {
            CoreError::Parse(_) => "cli.error.snapshot_parse",
            CoreError::InvalidPath(_) => "cli.error.input_read",
            _ => "cli.error.snapshot_read",
        };
        t_fmt(key, &[("error", &e.to_string())])
    })?;
    let output = format_restore_output(&original);
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}

/// Scan, filter and print skills, honoring the --global/--project flags.
/// 扫描、过滤并输出技能，遵循 --global/--project 旗标。
fn run_skill(global: bool, project: bool, home: Option<&Path>) -> Result<(), String> {
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
fn run_skill_with_home(global: bool, project: bool, home: &Path) -> Result<(), String> {
    reload_skills_with_home(&current_dir()?, home)
        .map_err(|e| t_fmt("cli.error.skill_scan", &[("error", &e.to_string())]))?;
    let skills = all_skills();
    let scope = match (global, project) {
        (true, true) => SkillScope::All,
        (true, false) => SkillScope::Global,
        (false, true) => SkillScope::Project,
        (false, false) => SkillScope::All,
    };
    let filtered = filter_skills(skills, scope);
    let output = format_skill_output(&filtered);
    pager::print_paged(&output).map_err(|e| t_fmt("cli.error.output", &[("error", &e.to_string())]))
}
