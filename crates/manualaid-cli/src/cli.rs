//! # Description
//! The clap-defined command line interface: the global `--lang` flag and
//! the `mask`/`restore`/`skill`/`init`/`dir` subcommands.
//! # 描述
//! clap 定义的命令行接口：全局 `--lang` 旗标，以及
//! `mask`/`restore`/`skill`/`init`/`dir` 子命令。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// The parsed command line arguments.
/// 解析后的命令行参数。
#[derive(Parser, Debug)]
#[command(
    name = "manualaid-cli",
    version,
    about = "ManualAid command line interface"
)]
pub struct Cli {
    /// Interface language code: en or zh-CN
    /// 界面语言代码：en 或 zh-CN
    #[arg(short, long, global = true, default_value = "en")]
    pub lang: String,
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The subcommands of the CLI.
/// CLI 的子命令。
#[derive(Subcommand, Debug)]
pub enum Command {
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
    /// Initialize the project and/or global `.ManualAid` folders. Shortcut
    /// for `dir --init`.
    /// 初始化项目/全局 `.ManualAid` 文件夹，等价于 `dir --init`。
    Init {
        /// Initialize the project folder only
        /// 仅初始化项目文件夹
        #[arg(long)]
        project: bool,
        /// Initialize the global folder only
        /// 仅初始化全局文件夹
        #[arg(long)]
        global: bool,
    },
    /// Manage the project and/or global `.ManualAid` folders.
    /// 管理项目/全局 `.ManualAid` 文件夹。
    #[command(group(
        clap::ArgGroup::new("dir_action")
            .required(true)
            .multiple(false)
            .args(["init", "view", "clean"])
    ))]
    Dir {
        /// Initialize the folders (same as the `init` command)
        /// 初始化文件夹（同 `init` 命令）
        #[arg(long)]
        init: bool,
        /// Show the `.ManualAid` file tree
        /// 显示 `.ManualAid` 文件树
        #[arg(long)]
        view: bool,
        /// Remove the `.ManualAid` directories (confirmation required unless
        /// `--yes`)
        /// 删除 `.ManualAid` 目录（除非带 `--yes`，否则需要确认）
        #[arg(long)]
        clean: bool,
        /// Project scope only
        /// 仅项目范围
        #[arg(long)]
        project: bool,
        /// Global scope only
        /// 仅全局范围
        #[arg(long)]
        global: bool,
        /// Max files shown per level; <= 0 means all
        /// 每层最多显示的文件数；小于等于 0 表示全部
        #[arg(long, allow_hyphen_values = true)]
        limit: Option<i64>,
        /// Max recursion depth; 0 shows only the root; < 0 means unlimited
        /// 最大递归深度；0 仅显示根目录；小于 0 表示不限制
        #[arg(long, allow_hyphen_values = true)]
        depth: Option<i64>,
        /// Skip the confirmation prompt when cleaning
        /// 清理时跳过确认提示
        #[arg(long)]
        yes: bool,
    },
}
