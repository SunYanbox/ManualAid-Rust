//! Idempotent creation of ManualAid's standard directories and files.
//! ManualAid 标准目录与文件的幂等创建。
//!
//! # Description
//! Missing targets are created with their default content; existing files
//! are never overwritten. The project `.ManualAid` directory is git-ignored,
//! so project-local files (such as `config.toml`) are never tracked.
//! # Test notes
//! The directory-creation error branch depends on OS state that tests
//! cannot construct portably (permission-denied paths); it is not required
//! to have high test coverage.
//! # 描述
//! 缺失的目标会被创建为默认内容；已存在的文件永不覆盖。项目的
//! `.ManualAid` 目录被 git 忽略，因此项目本地文件（如 `config.toml`）
//! 不会被跟踪。
//! # 测试说明
//! 目录创建的错误分支依赖测试无法跨平台构造的 OS 状态（权限拒绝路径）；
//! 该分支不要求高测试覆盖率。

use std::path::Path;

use crate::error::{CoreError, CoreResult};
use crate::file_io;
use crate::user_dir;

/// A file that the `ensure_*` functions create when missing. Its parent
/// directory is `<base>/.ManualAid`, where `base` is the user home or the
/// project root.
///
/// 缺失时由 `ensure_*` 函数创建的文件。其父目录为 `<base>/.ManualAid`，
/// `base` 为用户主目录或项目根。
struct FileTarget {
    /// File name within `.ManualAid`.
    /// `.ManualAid` 内的文件名。
    name: &'static str,
    /// Exact content written for a new file (config files carry a minimal
    /// valid TOML document).
    /// 新建文件写入的确切内容（配置文件为最小的合法 TOML 文档）。
    content: &'static str,
}

/// Content of the generated home `.ManualAid/config.toml`: commented
/// templates for every key loaded by the CLI loop and [`crate::privacy`].
/// 生成的 `~/.ManualAid/config.toml` 的内容：CLI loop 与 [`crate::privacy`]
/// 实际加载的全部配置项的注释模板。
pub const DEFAULT_GLOBAL_CONFIG_CONTENT: &str = concat!(
    "# ManualAid 全局配置文件（~/.ManualAid/config.toml）。\n",
    "# 由 `manualaid init` 自动生成；文件已存在时不会被覆盖。\n",
    "# 项目配置（<项目根>/.ManualAid/config.toml）按 key 覆盖全局配置。\n",
    "\n",
    "# 界面语言：`en` 或 `zh-CN`，默认 `en`。\n",
    "# 工具调用格式：`auto` / `xml` / `json-codeblock`，默认 `auto`（自动探测）。\n",
    "# 复制到剪贴板的结果文本最大字符数，默认 50000。\n",
    "# 是否自动加载上下文文件（AGENTS.md 等），默认 true。\n",
    "[global]\n",
    "# lang = \"en\"\n",
    "# tool_call_format = \"auto\"\n",
    "# max_result_chars = 50000\n",
    "# context_auto_load = true\n",
    "\n",
    "# 基础工具开关：控制工具是否出现在提示词与工具路由中，默认全部启用。\n",
    "[tools]\n",
    "# shell = true\n",
    "# read = true\n",
    "# edit = true\n",
    "# write = true\n",
    "# skill = true\n",
    "\n",
    "# 免审核白名单：命中即无需用户交互直接执行（精确匹配或 `*` 通配符；\n",
    "# 含 `;`、`&`、`|` 连接符的命令不会命中白名单）。\n",
    "[permissions]\n",
    "# allow_commands = [\"gh pr view *\", \"cargo fmt -- --check\"]\n",
    "\n",
    "# 隐私掩码扩展 —— 正则匹配（可选）。\n",
    "# 键为类型名，值为正则字符串；匹配到的文本在发送给 LLM 前会被替换为占位符。\n",
    "[privacy_mask_extension.regex]\n",
    "# ExamApiKey = \"^sk-[A-Za-z0-9]{7}$\"\n",
    "\n",
    "# 隐私掩码扩展 —— 字面量匹配（可选）。\n",
    "# 键为类型名，值为普通文本；该值会在输入任意位置做非正则的子串匹配。\n",
    "[privacy_mask_extension.literal]\n",
    "# UserName = \"Alice\"\n",
);

/// Content of the generated project `.ManualAid/config.toml`: the `[skill]`
/// table used by [`crate::skill`] for enable/disable state plus commented
/// templates for every key loaded by the CLI loop and [`crate::privacy`].
/// Also the base document used by [`crate::skill::set_enabled`] when the
/// config file is missing or blank.
/// 生成的项目 `.ManualAid/config.toml` 的内容：供 [`crate::skill`] 读写
/// 启用/禁用状态的 `[skill]` 表，以及 CLI loop 与 [`crate::privacy`] 实际
/// 加载的全部配置项的注释模板。配置文件缺失或空白时，也作为
/// [`crate::skill`] 创建文件的基础文档。
pub const DEFAULT_PROJECT_CONFIG_CONTENT: &str = concat!(
    "# ManualAid 项目配置文件（<项目根>/.ManualAid/config.toml）。\n",
    "# 由 `manualaid init` 自动生成；文件已存在时不会被覆盖。\n",
    "# 项目配置按 key 覆盖全局配置；目录内的 .gitignore 保证本文件不被提交。\n",
    "\n",
    "# 界面语言：`en` 或 `zh-CN`；未设置时使用全局配置。\n",
    "# 工具调用格式：`auto` / `xml` / `json-codeblock`；未设置时使用全局配置。\n",
    "# 复制到剪贴板的结果文本最大字符数，默认 50000。\n",
    "# 是否自动加载上下文文件（AGENTS.md 等），默认 true。\n",
    "[global]\n",
    "# lang = \"zh-CN\"\n",
    "# tool_call_format = \"auto\"\n",
    "# max_result_chars = 50000\n",
    "# context_auto_load = true\n",
    "\n",
    "# 基础工具开关：控制工具是否出现在提示词与工具路由中，默认全部启用。\n",
    "[tools]\n",
    "# shell = true\n",
    "# read = true\n",
    "# edit = true\n",
    "# write = true\n",
    "# skill = true\n",
    "\n",
    "# 免审核白名单：命中即无需用户交互直接执行（精确匹配或 `*` 通配符；\n",
    "# 含 `;`、`&`、`|` 连接符的命令不会命中白名单）。\n",
    "[permissions]\n",
    "# allow_commands = [\"gh pr view *\", \"cargo fmt -- --check\"]\n",
    "\n",
    "# 技能启用/禁用状态（可选，一般由 skill 相关功能读写）。\n",
    "# 键为技能目录的绝对路径（路径分隔符用 /，键需加引号），值为布尔值。\n",
    "[skill]\n",
    "# \"/absolute/path/to/skill\" = true\n",
    "\n",
    "# 隐私掩码扩展 —— 正则匹配（可选；与全局配置按 key 合并，项目覆盖全局）。\n",
    "# 键为类型名，值为正则字符串；匹配到的文本在发送给 LLM 前会被替换为占位符。\n",
    "[privacy_mask_extension.regex]\n",
    "# ExamApiKey = \"^sk-[A-Za-z0-9]{7}$\"\n",
    "\n",
    "# 隐私掩码扩展 —— 字面量匹配（可选）。\n",
    "# 键为类型名，值为普通文本；该值会在输入任意位置做非正则的子串匹配。\n",
    "[privacy_mask_extension.literal]\n",
    "# UserName = \"Alice\"\n",
);

/// Content of the generated project `.ManualAid/.gitignore`; ignores every
/// file in the directory so the project config stays untracked.
/// 生成的项目 `.ManualAid/.gitignore` 的内容；忽略目录内所有文件，使项目
/// 配置不被跟踪。
pub const GITIGNORE_CONTENT: &str = "# Automatically generated by ManualAid\n*\n";

/// Files created under `<home>/.ManualAid`.
/// 创建于 `<home>/.ManualAid` 下的文件。
const GLOBAL_FILE_TARGETS: &[FileTarget] = &[FileTarget {
    name: "config.toml",
    content: DEFAULT_GLOBAL_CONFIG_CONTENT,
}];

/// Files created under `<project>/.ManualAid`.
/// 创建于 `<project>/.ManualAid` 下的文件。
const PROJECT_FILE_TARGETS: &[FileTarget] = &[
    FileTarget {
        name: "config.toml",
        content: DEFAULT_PROJECT_CONFIG_CONTENT,
    },
    FileTarget {
        name: ".gitignore",
        content: GITIGNORE_CONTENT,
    },
];

/// Idempotently create ManualAid's standard directories and files: the
/// missing targets listed in the module documentation are created with their
/// default content. Existing files are never overwritten.
/// 幂等地创建 ManualAid 的标准目录与文件：缺失时按模块文档列出的目标
/// 创建默认内容；已存在的文件永不覆盖。
pub fn ensure_manualaid_dirs(project_root: &Path) -> CoreResult<()> {
    let home = user_dir::home_dir()?;
    ensure_manualaid_dirs_with_home(project_root, &home)
}

/// Like [`ensure_manualaid_dirs`] with an explicit home directory, used by
/// tests to avoid touching the real user home. Hidden from docs.
/// 同 [`ensure_manualaid_dirs`]，但以显式指定的主目录代替真实用户主目录，
/// 供测试避免触碰真实主目录。文档中隐藏。
#[doc(hidden)]
pub fn ensure_manualaid_dirs_with_home(project_root: &Path, home: &Path) -> CoreResult<()> {
    ensure_project_manualaid_dir(project_root)?;
    ensure_global_manualaid_dir(home)
}

/// Idempotently create only the project `.ManualAid` directory: the project
/// `config.toml` and `.gitignore`. The global directory is untouched.
/// 幂等地只创建项目 `.ManualAid` 目录：项目 `config.toml` 与 `.gitignore`。
/// 不触碰全局目录。
pub fn ensure_project_manualaid_dir(project_root: &Path) -> CoreResult<()> {
    let root = std::path::absolute(project_root).map_err(CoreError::from)?;
    ensure_manualaid_dir_files(&root, PROJECT_FILE_TARGETS)
}

/// Idempotently create only the global `.ManualAid` directory: the global
/// `config.toml`. The project directory is untouched.
/// 幂等地只创建全局 `.ManualAid` 目录：全局 `config.toml`。不触碰项目目录。
pub fn ensure_global_manualaid_dir(home: &Path) -> CoreResult<()> {
    ensure_manualaid_dir_files(home, GLOBAL_FILE_TARGETS)
}

/// Create `<base>/.ManualAid` and the given files when missing, never
/// overwriting existing files.
/// 创建 `<base>/.ManualAid` 与给定文件；已存在的文件永不覆盖。
fn ensure_manualaid_dir_files(base: &Path, targets: &[FileTarget]) -> CoreResult<()> {
    let dir = base.join(".ManualAid");
    std::fs::create_dir_all(&dir)
        .map_err(|e| CoreError::Io(format!("cannot create directory `{}`: {e}", dir.display())))?;

    for target in targets {
        let file = dir.join(target.name);
        file_io::with_file_lock(&file, || {
            if !file.exists() {
                std::fs::write(&file, target.content).map_err(CoreError::from)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Statistics collected before a `.ManualAid` directory is removed.
/// 删除 `.ManualAid` 目录前统计得到的数值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanReport {
    /// Number of files (including symlinks) under the directory.
    /// 目录下的文件数（含符号链接）。
    pub files: u64,
    /// Sum of file sizes in bytes; metadata read failures count as 0.
    /// 文件字节数总和；元数据读取失败按 0 计。
    pub bytes: u64,
}

/// Remove `<base>/.ManualAid` entirely when it exists, returning the
/// collected [`CleanReport`], or `None` when the directory does not exist.
/// 当 `<base>/.ManualAid` 存在时整体删除，返回统计的 [`CleanReport`]；
/// 目录不存在时返回 `None`。
///
/// # Description
/// A `.ManualAid` path that exists but is not a directory is an
/// `InvalidPath` error. Only the exact `base.join(".ManualAid")` path is
/// touched; `base` itself is never removed.
/// # 描述
/// `.ManualAid` 路径存在但不是目录时报`InvalidPath`。只操作精确的
/// `base.join(".ManualAid")` 路径，绝不删除`base` 本身。
pub fn clean_manualaid_dir(base: &Path) -> CoreResult<Option<CleanReport>> {
    let target = base.join(".ManualAid");
    if !target.exists() {
        return Ok(None);
    }
    if !target.is_dir() {
        return Err(CoreError::InvalidPath(format!(
            "`{}` exists but is not a directory",
            target.display()
        )));
    }
    let report = collect_clean_report(&target)?;
    std::fs::remove_dir_all(&target).map_err(CoreError::from)?;
    Ok(Some(report))
}

/// Recursively count files and sum their sizes under `dir`.
/// 递归统计 `dir` 下的文件数与其大小总和。
fn collect_clean_report(dir: &Path) -> CoreResult<CleanReport> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    for entry in std::fs::read_dir(dir).map_err(CoreError::from)? {
        let entry = entry.map_err(CoreError::from)?;
        let file_type = entry.file_type().map_err(CoreError::from)?;
        if file_type.is_dir() {
            let sub = collect_clean_report(&entry.path())?;
            files += sub.files;
            bytes += sub.bytes;
        } else {
            files += 1;
            bytes += entry.metadata().map_or(0, |meta| meta.len());
        }
    }
    Ok(CleanReport { files, bytes })
}
