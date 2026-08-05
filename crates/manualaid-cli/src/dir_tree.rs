//! # Description
//! Bounded, depth-first rendering of a directory tree for CLI output.
//!
//! [`format_dir_tree`] prints the root line followed by box-drawing tree
//! lines. Directories are listed before files, both sorted by name
//! case-insensitively. When `per_level_limit` is `Some(n)`, each level shows
//! at most `n` files and `3 * n` directories, and the whole tree shows at
//! most `64 * n` non-root directories; exceeding entries are summarized with
//! localized `… N more` lines. A limit of `None` disables every budget.
//! `depth` limits recursion: `Some(0)` prints only the root, `Some(d)` stops
//! at depth `d`, and `None` means unlimited.
//!
//! # Test notes
//! The error branches of `read_dir` / `file_type` on subdirectories depend
//! on OS state that tests cannot construct portably (permission-denied
//! paths); unreadable subdirectories are shown without children, and these
//! branches are not required to have high test coverage.
//! # 描述
//! 用于 CLI 输出的带预算的深度优先目录树渲染。
//!
//! [`format_dir_tree`] 输出根行与箱线风格的树行。目录排在文件之前，各自按
//! 名称（大小写不敏感）排序。当 `per_level_limit` 为 `Some(n)` 时，每层最多
//! 显示 `n` 个文件与 `3 * n` 个目录，全树最多显示 `64 * n` 个非根目录；
//! 超出的条目用本地化的 `… N more` 行汇总。`None` 表示不设任何预算。
//! `depth` 限制递归：`Some(0)` 只打印根，`Some(d)` 在深度 `d` 停止，
//! `None` 表示不限制。
//!
//! # 测试说明
//! 子目录 `read_dir` / `file_type` 的错误分支依赖测试无法跨平台构造的 OS
//! 状态（权限拒绝路径）；不可读子目录只显示本身不展开，这些分支不要求高
//! 测试覆盖率。

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use manualaid_core::error::{CoreError, CoreResult};

use crate::t_fmt;

/// Default maximum recursion depth when `--depth` is not given.
/// 未指定 `--depth` 时的默认最大递归深度。
pub const DEFAULT_VIEW_DEPTH: usize = 3;

/// Default per-level file limit when `--limit` is not given.
/// 未指定 `--limit` 时的默认每层文件上限。
pub const DEFAULT_VIEW_LIMIT: usize = 7;

/// Multiplier for the per-level directory budget (`3 * limit`).
/// 每层目录预算的倍数（`3 * limit`）。
pub const DIRS_PER_LEVEL_FACTOR: usize = 3;

/// Multiplier for the whole-tree directory budget (`64 * limit`).
/// 全树目录预算的倍数（`64 * limit`）。
pub const TOTAL_DIRS_FACTOR: usize = 64;

/// # Description
/// View configuration for [`format_dir_tree`]. `None` means unlimited.
/// # 描述
/// [`format_dir_tree`] 的查看配置。`None` 表示不限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirViewConfig {
    /// Maximum recursion depth; `Some(0)` prints only the root.
    /// 最大递归深度；`Some(0)` 只打印根目录。
    pub depth: Option<usize>,
    /// Per-level file limit; directories use `3 * limit` per level and the
    /// tree uses `64 * limit` total.
    /// 每层文件上限；目录按每层 `3 * limit`、全树 `64 * limit` 计算。
    pub per_level_limit: Option<usize>,
}

impl Default for DirViewConfig {
    fn default() -> Self {
        Self {
            depth: Some(DEFAULT_VIEW_DEPTH),
            per_level_limit: Some(DEFAULT_VIEW_LIMIT),
        }
    }
}

/// A single sorted directory entry.
/// 单个已排序的目录条目。
struct TreeEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

/// Global directory budget tracking during a depth-first walk.
/// 深度优先遍历期间的全树目录预算跟踪。
struct TreeState {
    total_dir_budget: Option<usize>,
    dirs_shown: usize,
}

/// # Description
/// Render `root` as a bounded depth-first tree. The first line is
/// `- <root path>`; later lines use `├──` / `└──` / `│` connectors, and
/// directories end with `/`. The root itself is always shown and never
/// counts toward the directory budget.
/// # 描述
/// 将 `root` 渲染为带预算的深度优先树。首行为 `- <根路径>`；后续行使用
/// `├──` / `└──` / `│` 连接符，目录以 `/` 结尾。根目录恒显示且不计入
/// 目录预算。
pub fn format_dir_tree(root: &Path, config: &DirViewConfig) -> CoreResult<String> {
    if !root.exists() {
        return Err(CoreError::NotFound(format!(
            "directory `{}` not found",
            root.display()
        )));
    }
    if !root.is_dir() {
        return Err(CoreError::InvalidPath(format!(
            "`{}` is not a directory",
            root.display()
        )));
    }
    let mut lines = vec![format!("- {}", root.display())];
    let (dirs, files) = read_entries(root)?;
    let mut state = TreeState {
        total_dir_budget: config
            .per_level_limit
            .map(|limit| limit.saturating_mul(TOTAL_DIRS_FACTOR)),
        dirs_shown: 0,
    };
    render_entries("", 1, dirs, files, config, &mut state, &mut lines)?;
    Ok(lines.join("\n"))
}

/// Read and split the entries of `dir`, skipping entries whose file type
/// cannot be queried.
/// 读取 `dir` 的条目并按目录/文件拆分，跳过无法查询文件类型的条目。
fn read_entries(dir: &Path) -> CoreResult<(Vec<TreeEntry>, Vec<TreeEntry>)> {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(CoreError::from)? {
        let entry = entry.map_err(CoreError::from)?;
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        let target = TreeEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path(),
            is_dir: file_type.is_dir(),
        };
        if target.is_dir {
            dirs.push(target);
        } else {
            files.push(target);
        }
    }
    Ok((dirs, files))
}

/// Read the children of `parent` and render them; an unreadable
/// subdirectory is shown without children.
/// 读取并渲染 `parent` 的子条目；不可读的子目录只显示本身不展开。
fn render_children(
    parent: &Path,
    prefix: &str,
    depth: usize,
    config: &DirViewConfig,
    state: &mut TreeState,
    lines: &mut Vec<String>,
) -> CoreResult<()> {
    let (dirs, files) = match read_entries(parent) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    render_entries(prefix, depth, dirs, files, config, state, lines)
}

/// Render the already-read entries at `depth`, applying the depth and budget
/// limits, then recurse into the shown directories.
/// 渲染 `depth` 层已读取的条目，应用深度与预算限制，然后递归进入显示的
/// 目录。
fn render_entries(
    prefix: &str,
    depth: usize,
    mut dirs: Vec<TreeEntry>,
    mut files: Vec<TreeEntry>,
    config: &DirViewConfig,
    state: &mut TreeState,
    lines: &mut Vec<String>,
) -> CoreResult<()> {
    if let Some(max_depth) = config.depth
        && depth > max_depth
    {
        return Ok(());
    }
    dirs.sort_by(compare_names);
    files.sort_by(compare_names);

    let dir_limit = config
        .per_level_limit
        .map(|limit| limit.saturating_mul(DIRS_PER_LEVEL_FACTOR));
    let mut dirs_shown = dirs.len();
    if let Some(limit) = dir_limit {
        dirs_shown = dirs_shown.min(limit);
    }
    if let Some(budget) = state.total_dir_budget {
        let remaining = budget.saturating_sub(state.dirs_shown);
        dirs_shown = dirs_shown.min(remaining);
    }
    state.dirs_shown += dirs_shown;

    let files_shown = match config.per_level_limit {
        Some(limit) => files.len().min(limit),
        None => files.len(),
    };

    let total_shown = dirs_shown + files_shown;
    let mut shown = 0usize;
    for entry in dirs.iter().take(dirs_shown) {
        let last = shown + 1 == total_shown;
        lines.push(format!(
            "{prefix}{}{}/",
            if last { "└── " } else { "├── " },
            entry.name
        ));
        let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
        render_children(&entry.path, &child_prefix, depth + 1, config, state, lines)?;
        shown += 1;
    }
    for entry in files.iter().take(files_shown) {
        let last = shown + 1 == total_shown;
        lines.push(format!(
            "{prefix}{}{}",
            if last { "└── " } else { "├── " },
            entry.name
        ));
        shown += 1;
    }

    let omitted_dirs = dirs.len() - dirs_shown;
    if omitted_dirs > 0 {
        lines.push(format!(
            "{prefix}  {}",
            t_fmt("cli.dir.more_dirs", &[("count", &omitted_dirs.to_string())])
        ));
    }
    let omitted_files = files.len() - files_shown;
    if omitted_files > 0 {
        lines.push(format!(
            "{prefix}  {}",
            t_fmt(
                "cli.dir.more_files",
                &[("count", &omitted_files.to_string())]
            )
        ));
    }
    Ok(())
}

/// Compare entries by lowercased name, then by raw name for stability.
/// 先按小写名称比较，再按原始名称比较以保证稳定排序。
fn compare_names(a: &TreeEntry, b: &TreeEntry) -> Ordering {
    a.name
        .to_lowercase()
        .cmp(&b.name.to_lowercase())
        .then_with(|| a.name.cmp(&b.name))
}
