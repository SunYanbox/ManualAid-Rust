//! Path candidate provider for `@` completion. A visible-path walk serves
//! ordinary searches, while ignored directories stay browsable as dimmed
//! entries: entering one lists its direct children on demand and caches the
//! result for the whole session.
//! `@` 补全的路径候选提供者。未忽略路径走常规遍历；被忽略目录仍可作为
//! 弱化条目浏览，进入时按需列举其直接子项并缓存整个会话。

use std::collections::HashMap;
use std::path::Path;

use ignore::WalkBuilder;

/// Maximum depth for the ordinary visible walk; deeper build outputs are not
/// useful search targets.
/// 常规未忽略遍历的最大深度；更深的构建产物不是有用的搜索目标。
const MAX_DEPTH: usize = 12;

/// Upper bound for one search result so a huge tree cannot flood the
/// suggestion list.
/// 单次搜索结果的条数上限，避免巨大的目录树淹没建议列表。
const MAX_MATCHES: usize = 200;

/// One path suggestion; `dimmed` means the entry is ignored by the project's
/// ignore rules but is still offered as a browsable target.
/// 一条路径建议；`dimmed` 表示该条目被项目忽略规则忽略，但仍可作为浏览
/// 目标显示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathEntry {
    /// Relative path from the project root, in platform-separator form.
    /// 相对项目根的路径，使用平台分隔符。
    pub label: String,
    /// Whether the entry is a directory.
    /// 条目是否为目录。
    pub is_dir: bool,
    /// Whether the entry is ignored by gitignore rules.
    /// 条目是否被 gitignore 规则忽略。
    pub dimmed: bool,
}

/// Session-cached visible entries plus an on-demand cache of browsed
/// directories.
/// 会话缓存的可见条目，以及按需浏览目录的缓存。
#[derive(Default)]
pub(crate) struct PathCandidates {
    root: std::path::PathBuf,
    visible: Vec<PathEntry>,
    /// Directory content caches keyed by the normalized relative directory
    /// path; the empty key is the project root and holds the top-level view.
    /// 以规范化的相对目录路径为键的目录内容缓存；空键表示项目根，保存
    /// 顶层视图。
    browsed: HashMap<String, Vec<PathEntry>>,
}

impl PathCandidates {
    /// Replace the cached view with a scan rooted at `root`.
    /// 以 `root` 为根重新扫描并替换缓存。
    pub(crate) fn scan(&mut self, root: &Path) {
        self.root = root.to_path_buf();
        self.visible = walk_visible(root);
        self.browsed.clear();
        // The empty key is the root view shown when the query is empty.
        // 空键是查询为空时显示的根视图。
        let root_children = list_children(root);
        self.browsed.insert(String::new(), root_children);
    }

    /// Return path suggestions for `query`. Empty queries list the top level;
    /// a trailing separator lists the direct children of the named directory;
    /// other queries search the visible entries plus top-level ignored
    /// directories so a dimmed directory can still be entered.
    /// 返回 `query` 对应的路径建议。空查询列出顶层；带尾分隔符时列出指定
    /// 目录的直接子项；其他查询搜索可见条目并额外匹配顶层被忽略目录，
    /// 使弱化目录仍可进入。
    pub(crate) fn filter(&mut self, query: &str) -> Vec<PathEntry> {
        let normalized = query.replace('\\', "/");
        if normalized.is_empty() {
            return self.children("");
        }
        if normalized.ends_with('/') {
            let dir_path = normalized.trim_end_matches('/');
            return self.children(dir_path);
        }

        let segments = query_segments(&normalized);
        if segments.len() == 1 {
            // A single-segment query searches the whole visible tree by its
            // final path component, so `@prompt` finds
            // `crates/i18n/locales/prompts.en.toml` as well as top-level
            // files. Top-level ignored directories stay reachable the same
            // way.
            // 单段查询按路径最后一段在整个可见树中搜索，`@prompt` 既能找到
            // 深层文件也能找到顶层文件。顶层被忽略目录同样可达。
            let needle = &segments[0];
            let mut matches: Vec<PathEntry> = self
                .visible
                .iter()
                .filter(|entry| basename_lower(&entry.label).starts_with(needle))
                .cloned()
                .collect();
            if let Some(top_level) = self.browsed.get("") {
                matches.extend(
                    top_level
                        .iter()
                        .filter(|entry| {
                            entry.dimmed && basename_lower(&entry.label).starts_with(needle)
                        })
                        .cloned(),
                );
            }
            matches.truncate(MAX_MATCHES);
            return matches;
        }

        // A multi-segment query such as `@crates/i` narrows to the direct
        // children of `crates` whose final component starts with `i`.
        // 多段查询（如 `@crates/i`）缩小到 `crates` 的直接子项，并按其
        // 最后一段以 `i` 开头过滤。
        let (dir_key, child_needle) = normalized.rsplit_once('/').expect("query has a separator");
        let mut matches: Vec<PathEntry> = self
            .children(dir_key)
            .into_iter()
            .filter(|entry| basename_lower(&entry.label).starts_with(child_needle))
            .collect();
        matches.truncate(MAX_MATCHES);
        matches
    }

    /// Return the cached direct children of the directory named by `dir_path`;
    /// scan and cache on first use. Labels for non-root directories are full
    /// relative paths so Tab can replace the active token without losing the
    /// directory prefix.
    /// 返回 `dir_path` 所表示目录的直接子项（已缓存）；首次使用时扫描并缓存。
    /// 非根目录的标签为完整相对路径，Tab 替换活动 token 时不会丢失目录前缀。
    fn children(&mut self, dir_path: &str) -> Vec<PathEntry> {
        if let Some(cached) = self.browsed.get(dir_path) {
            return cached.clone();
        }
        let absolute = if dir_path.is_empty() {
            self.root.clone()
        } else {
            self.root.join(dir_path)
        };
        let mut entries = list_children(&absolute);
        if !dir_path.is_empty() {
            let separator = std::path::MAIN_SEPARATOR.to_string();
            for entry in &mut entries {
                entry.label = format!("{}{}{}", dir_path, separator, entry.label);
            }
        }
        self.browsed.insert(dir_path.to_owned(), entries.clone());
        entries
    }
}

/// Collect the visible (non-ignored) entries under `root`, depth-capped,
/// without pruning ignored directories so they can still be browsed later.
/// 收集 `root` 下未忽略的条目并限制深度；不剪枝被忽略目录，以便后续
/// 仍可浏览。
fn walk_visible(root: &Path) -> Vec<PathEntry> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.require_git(false);
    builder.git_ignore(true);
    builder.git_global(false);
    builder.parents(false);
    builder.max_depth(Some(MAX_DEPTH));
    builder.sort_by_file_name(|left, right| left.cmp(right));
    // Only `.git` itself is never a useful completion target.
    // 只有 `.git` 自身永远不是有用的补全目标。
    builder.filter_entry(|entry| entry.file_name().to_string_lossy() != ".git");

    let mut entries = Vec::new();
    for entry in builder.build() {
        let Ok(entry) = entry else { continue };
        if entry.depth() == 0 {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        let is_dir = entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir());
        entries.push(PathEntry {
            label: relative.to_string_lossy().into_owned(),
            is_dir,
            dimmed: false,
        });
    }
    entries
}

/// List the direct children of `dir` and mark ignored ones as dimmed. The
/// list is a single-level scan, so entering an ignored directory never
/// walks a huge subtree in one step.
/// 列出 `dir` 的直接子项并标记被忽略项为弱化。这是单层扫描，进入被忽略
/// 目录不会一次遍历巨大的子树。
fn list_children(dir: &Path) -> Vec<PathEntry> {
    /// Collect one level of child names together with their directory flag.
    /// 收集一层子项名称及其是否目录的标志。
    fn collect(root: &Path, git_ignore: bool) -> HashMap<String, bool> {
        let mut builder = WalkBuilder::new(root);
        builder.hidden(false);
        builder.require_git(false);
        builder.git_ignore(git_ignore);
        builder.git_global(false);
        builder.parents(false);
        builder.max_depth(Some(1));
        builder.filter_entry(|entry| entry.file_name().to_string_lossy() != ".git");

        let mut children = HashMap::new();
        for entry in builder.build().flatten() {
            if entry.depth() == 0 {
                continue;
            }
            let Some(name) = entry.file_name().to_str() else {
                continue;
            };
            let is_dir = entry
                .file_type()
                .is_some_and(|file_type| file_type.is_dir());
            children.insert(name.to_owned(), is_dir);
        }
        children
    }

    let full = collect(dir, false);
    let visible = collect(dir, true);
    let mut entries: Vec<PathEntry> = full
        .into_iter()
        .map(|(name, is_dir)| PathEntry {
            dimmed: !visible.contains_key(&name),
            label: name,
            is_dir,
        })
        .collect();
    entries.sort_by(|left, right| left.label.cmp(&right.label));
    entries
}

/// Lowercase query segments; both separators are accepted so Windows users
/// can type `/` and `\` interchangeably.
/// 查询切段并转小写；两种分隔符都可接受，让 Windows 用户可混用 `/` 与
/// `\`。
fn query_segments(text: &str) -> Vec<String> {
    text.replace('\\', "/")
        .split('/')
        .map(str::to_lowercase)
        .collect()
}

/// Lowercase final path component of `path`.
/// 返回 `path` 最后一段的小写形式。
fn basename_lower(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
