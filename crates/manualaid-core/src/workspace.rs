//! Workspace path helpers: normalization, containment checks and exemption
//! merging, used by the audit layer without requiring paths to exist.
//! 工作区路径辅助：归一化、包含关系检查与豁免合并，供审计层使用，
//! 不要求路径真实存在于磁盘。
//!
//! # Description
//! `normalize_path` resolves relative paths against the current working
//! directory and strips `.` / `..` lexically, so paths that do not exist yet
//! (e.g. a file about to be written) can still be compared. Containment
//! checks canonicalize both sides when possible (resolving symlinks) and
//! fall back to lexical comparison when the target does not exist.
//! # 描述
//! `normalize_path` 把相对路径基于当前工作目录解析，并在词法层面去除
//! `.` / `..` 组件，因此尚不存在的路径（例如待写入的文件）也能参与比较。
//! 包含关系检查在可能时先对两侧做 canonicalize（解析符号链接），目标
//! 不存在时回退到词法比较。

use std::path::{Component, Path, PathBuf};

/// Normalize a path without requiring it to exist on disk.
/// 在不要求路径存在于磁盘上的情况下规范化路径。
///
/// # Description
/// Relative paths are resolved against the current working directory.
/// `.` and `..` components are removed lexically; `..` above the root is
/// capped at the root. Returns the normalized absolute path.
/// # 描述
/// 相对路径基于当前工作目录解析。`.` 与 `..` 组件在词法层面被去除；
/// 超出根目录的 `..` 会被限制在根目录。返回规范化后的绝对路径。
pub fn normalize_path(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    lexical_normalize(&joined)
}

/// Check whether `path` is inside the workspace rooted at `workspace_root`.
/// 检查 `path` 是否在以 `workspace_root` 为根的工作区内部。
///
/// # Description
/// Both sides are canonicalized first when they exist, so symlinks and
/// `.` / `..` components do not affect the result; when the target does not
/// exist yet (e.g. a file about to be written) the comparison falls back to
/// lexical containment on normalized absolute paths.
/// # 描述
/// 两侧存在时先做 canonicalize，使符号链接与 `.` / `..` 组件不影响结果；
/// 目标尚不存在（例如待写入的文件）时，回退为对规范化绝对路径的词法
/// 包含比较。
pub fn is_within_workspace(path: &Path, workspace_root: &Path) -> bool {
    starts_with_ci(&comparable(path), &comparable(workspace_root))
}

/// Check whether `path` is a descendant of any entry in `exempt_paths`.
/// 检查 `path` 是否为 `exempt_paths` 中任一条目的后代路径。
///
/// # Description
/// Existing paths are canonicalized before comparison (resolving symlinks
/// and `..`); entries and targets that do not exist yet fall back to
/// lexically normalized absolute paths.
/// # 描述
/// 比较前对已存在的路径做 canonicalize（解析符号链接与 `..`）；尚不存在
/// 的条目与目标路径回退到词法归一化的绝对路径。
pub fn is_exempt_path(path: &Path, exempt_paths: &[PathBuf]) -> bool {
    let canonical = comparable(path);
    exempt_paths
        .iter()
        .any(|exempt| starts_with_ci(&canonical, &comparable(exempt)))
}

/// Merge workspace-level and global exemption lists, deduplicated and
/// sorted for deterministic output.
/// 合并工作区级与全局豁免列表，去重并排序，保证输出确定。
pub fn merge_exempt_paths(workspace_exempt: &[PathBuf], global_exempt: &[PathBuf]) -> Vec<PathBuf> {
    let mut merged: Vec<PathBuf> = workspace_exempt
        .iter()
        .chain(global_exempt.iter())
        .cloned()
        .collect();
    merged.sort();
    let mut deduped: Vec<PathBuf> = Vec::with_capacity(merged.len());
    for path in merged {
        if !deduped
            .iter()
            .any(|existing| paths_equal_ci(existing, &path))
        {
            deduped.push(path);
        }
    }
    deduped
}

/// Component-wise `starts_with` that ignores ASCII case on Windows, where
/// `Path::starts_with` is case-sensitive even though the filesystem is not
/// (a tool call may spell `E:\Project...` differently from the canonical
/// workspace root).
/// 逐组件比较的 `starts_with`；Windows 上忽略 ASCII 大小写（文件系统不区分
/// 大小写，但 `Path::starts_with` 仍然区分，工具调用对工作区根目录的写法
/// 可能大小写不同）。
fn starts_with_ci(path: &Path, prefix: &Path) -> bool {
    let path: Vec<_> = path.components().collect();
    let prefix: Vec<_> = prefix.components().collect();
    path.len() >= prefix.len()
        && path[..prefix.len()]
            .iter()
            .zip(&prefix)
            .all(|(a, b)| component_eq(a, b))
}

/// Whether two paths consist of the same components; case-insensitive on
/// Windows, exact elsewhere.
/// 判断两条路径是否由相同组件构成；Windows 上忽略大小写，其余平台精确比较。
fn paths_equal_ci(a: &Path, b: &Path) -> bool {
    let a: Vec<_> = a.components().collect();
    let b: Vec<_> = b.components().collect();
    a.len() == b.len() && a.iter().zip(&b).all(|(x, y)| component_eq(x, y))
}

/// Compare two path components, ignoring ASCII case on Windows.
/// 比较两个路径组件；Windows 上忽略 ASCII 大小写。
fn component_eq(a: &Component<'_>, b: &Component<'_>) -> bool {
    #[cfg(windows)]
    {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}

/// Remove `.` and `..` components lexically from `path`.
/// 在词法层面去除 `path` 中的 `.` 与 `..` 组件。
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // pop() returns false when already at the root, effectively
                // capping `..` above the root.
                // pop() 在已是根目录时返回 false，相当于把超出根目录的 `..` 截断。
                let _ = result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// Resolve a path for containment comparison: canonicalize the deepest
/// existing ancestor and re-append the still-missing tail. Canonicalization
/// resolves symlinks and junctions, so a nonexistent target (e.g. a file
/// about to be written) must share the same resolved root as the workspace;
/// comparing its raw lexical path against a canonicalized root would wrongly
/// report it as outside (the GitHub Actions Windows temp directory is a
/// junction). Falls back to lexical normalization when no ancestor exists.
/// 解析用于包含比较的路径：对最深的已存在祖先做 canonicalize（解析符号链接与
/// junction），再重新拼接尚未存在的尾部。尚不存在的目标（例如待写入的文件）
/// 必须与工作区共享同一解析根；若将原始词法路径与已解析的工作区根比较，
/// 会把目标误判为工作区之外（GitHub Actions 的 Windows 临时目录即为 junction）。
/// 当没有任何祖先存在时回退到词法归一化。
fn comparable(path: &Path) -> PathBuf {
    let mut missing: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cursor = path;
    loop {
        match cursor.canonicalize() {
            Ok(canonical) => {
                let mut resolved = strip_verbatim(canonical);
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return resolved;
            }
            Err(_) => match cursor.file_name() {
                Some(name) => {
                    missing.push(name);
                    match cursor.parent() {
                        Some(parent) => cursor = parent,
                        None => return normalize_path(path),
                    }
                }
                None => return normalize_path(path),
            },
        }
    }
}

/// Remove the `\\?\` verbatim prefix that `canonicalize` adds on Windows
/// for extended-length paths, so lexically normalized fallback paths and
/// canonicalized paths share the same component form.
/// 去掉 Windows 上 `canonicalize` 为长路径添加的 `\\?\` 前缀，使词法
/// 归一化回退路径与 canonicalize 路径具有相同的组件形式。
#[cfg(windows)]
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => path,
    }
}

#[cfg(not(windows))]
fn strip_verbatim(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_makes_relative_paths_absolute() {
        let normalized = normalize_path(Path::new("a/b"));
        assert!(normalized.is_absolute());
        assert!(normalized.ends_with("a/b"));
    }

    #[test]
    fn normalize_strips_dot_and_parent_components() {
        let base = std::env::current_dir().unwrap();
        let input = base.join("a/./b/../c");
        let normalized = normalize_path(&input);
        assert_eq!(normalized, base.join("a/c"));
    }

    #[test]
    fn lexical_normalize_skips_dot_components_and_keeps_plain_paths() {
        assert_eq!(lexical_normalize(Path::new("a/./b")), PathBuf::from("a/b"));
        assert_eq!(
            strip_verbatim(PathBuf::from("plain/path")),
            PathBuf::from("plain/path")
        );
    }

    #[test]
    fn normalize_caps_parent_above_root() {
        #[cfg(windows)]
        let root = Path::new("C:\\");
        #[cfg(not(windows))]
        let root = Path::new("/");
        let normalized = normalize_path(&root.join("..").join("x"));
        assert_eq!(normalized, root.join("x"));
    }

    #[test]
    fn within_workspace_matches_nested_paths() {
        let root = std::env::temp_dir();
        assert!(is_within_workspace(&root.join("a.txt"), &root));
        assert!(is_within_workspace(&root.join("sub"), &root));
        assert!(!is_within_workspace(
            &root.parent().unwrap().join("other"),
            &root
        ));
    }

    #[test]
    fn within_workspace_handles_nonexistent_target() {
        let root = std::env::temp_dir();
        let target = root.join("manualaid-ws-nonexistent").join("new.txt");
        assert!(is_within_workspace(&target, &root));
    }

    #[cfg(windows)]
    #[test]
    fn within_workspace_resolves_nonexistent_target_through_junction() {
        use std::os::windows::fs::symlink_dir;
        let root =
            std::env::temp_dir().join(format!("manualaid-ws-junction-{}", std::process::id()));
        let real = root.join("real");
        let link = root.join("link");
        std::fs::create_dir_all(&real).expect("create real dir");
        // Directory symlink creation needs Developer Mode or an elevated
        // shell; skip when the OS denies it so the suite still runs on
        // unprivileged machines.
        // 创建目录符号链接需要开发者模式或提权终端；操作系统拒绝时跳过，
        // 保证测试套件在未提权机器上仍可运行。
        if symlink_dir(&real, &link).is_err() {
            return;
        }
        let target = link.join("new.txt");
        assert!(is_within_workspace(&target, &link));
        let _ = std::fs::remove_dir(&link);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(not(windows))]
    #[test]
    fn within_workspace_resolves_nonexistent_target_through_symlink() {
        use std::os::unix::fs::symlink;
        let root =
            std::env::temp_dir().join(format!("manualaid-ws-symlink-{}", std::process::id()));
        let real = root.join("real");
        let link = root.join("link");
        std::fs::create_dir_all(&real).expect("create real dir");
        symlink(&real, &link).expect("create symlink");
        let target = link.join("new.txt");
        assert!(is_within_workspace(&target, &link));
        let _ = std::fs::remove_dir(&link);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn exempt_path_matches_descendants_only() {
        let root = std::env::temp_dir();
        let exempt = root.join("manualaid-exempt");
        std::fs::create_dir_all(&exempt).expect("create exempt dir");
        assert!(is_exempt_path(
            &exempt.join("x.txt"),
            std::slice::from_ref(&exempt)
        ));
        assert!(!is_exempt_path(&root.join("other.txt"), &[exempt]));
    }

    #[test]
    fn merge_exempt_paths_deduplicates_and_sorts() {
        let a = PathBuf::from("/b");
        let b = PathBuf::from("/a");
        let merged = merge_exempt_paths(&[a.clone(), b.clone()], &[b, PathBuf::from("/c")]);
        assert_eq!(
            merged,
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn within_workspace_ignores_ascii_case_on_windows() {
        let root = std::env::temp_dir();
        let target = root.join("manualaid-ws-case").join("new.txt");
        let variant = PathBuf::from(target.to_string_lossy().to_lowercase());
        assert!(is_within_workspace(&variant, &root));
    }

    #[cfg(windows)]
    #[test]
    fn exempt_path_matches_ignoring_ascii_case_on_windows() {
        let root = std::env::temp_dir();
        let exempt = root.join("manualaid-exempt-ci");
        std::fs::create_dir_all(&exempt).expect("create exempt dir");
        let variant = PathBuf::from(exempt.to_string_lossy().to_lowercase());
        assert!(is_exempt_path(
            &variant.join("x.txt"),
            std::slice::from_ref(&exempt)
        ));
    }

    #[cfg(windows)]
    #[test]
    fn merge_exempt_paths_dedups_ignoring_ascii_case_on_windows() {
        let merged = merge_exempt_paths(
            &[PathBuf::from(r"C:\Foo\Bar")],
            &[PathBuf::from(r"c:\foo\bar")],
        );
        assert_eq!(merged.len(), 1);
    }
}
