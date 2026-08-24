//! The built-in translation wrapper library based on the rust_i18n crate for ManualAid.
//! 用于 ManualAid 的基于 rust_i18n crate 的翻译封装内置库。
//!
//! # Description
//! `rust_i18n::i18n!()` must be called exactly once in the crate graph —
//! here in the library crate — because `t!` expands to
//! `crate::_rust_i18n_t!(...)` which requires `i18n!()` to have run in the
//! same crate.  Calling it in both lib and bin causes macro-resolution
//! conflicts.  The binary crate uses `i18n::t_str()` instead.
//! # 描述
//! `rust_i18n::i18n!()` 必须在 crate 图中且仅调用一次 —
//! 这里放在库 crate 中 — 因为 `t!` 会展开为
//! `crate::_rust_i18n_t!(...)`，这要求 `i18n!()` 必须在同一个 crate 中
//! 执行过。如果在 lib 和 bin 中同时调用会导致宏解析冲突。
//! 二进制 crate 改为使用 `i18n::t_str()`。
mod init;
use crate::init::_rust_i18n_try_translate;

pub use rust_i18n::t;

/// Set the process-wide locale and refresh the ChangeLog language
/// selection. The current ChangeLog only provides Chinese, so every
/// locale falls back to the Chinese text.
/// 设置进程级 locale 并刷新 ChangeLog 语言选择。当前 ChangeLog 仅提供
/// 中文，因此任何 locale 都回退到中文文本。
pub fn set_locale(locale: &str) {
    rust_i18n::set_locale(locale);
}

/// Translate a key via the library's i18n backend.
/// 通过库的 i18n 后端翻译一个键值。
///
/// # Description
/// This function exists primarily for the binary crate (`main.rs`), which is a
/// separate crate and cannot directly use the `t!` macro: `t!` expands to
/// `crate::_rust_i18n_t!(…)`, which requires the calling crate to have invoked
/// `rust_i18n::i18n!()` — calling it in both lib and bin causes macro-resolution
/// conflicts, so only the library crate owns that invocation.
///
/// Inside the library crate `t!` works fine from sub-modules (e.g.
/// `println!("{{}}", t!("key"))`), but this function deliberately avoids using
/// `t!` internally: doing so would re-enter the proc-macro expansion chain
/// (`t!` → `_rust_i18n_t!` → `rust_i18n::_tr!`) which can stall early-phase
/// resolution for other `t!` call-sites in the same compilation unit.
/// Calling `_rust_i18n_try_translate` directly is equivalent and avoids that
/// hazard.
///
/// If no translation is found, the key itself is returned.
/// # 描述
/// 此函数主要为二进制 crate（`main.rs`）而存在，因为二进制 crate 是一个
/// 独立的 crate，无法直接使用 `t!` 宏：`t!` 会展开为
/// `crate::_rust_i18n_t!(…)`，这要求调用 crate 必须已经调用了
/// `rust_i18n::i18n!()` —— 如果在 lib 和 bin 中同时调用会导致宏解析
/// 冲突，因此只有库 crate 拥有该调用的所有权。
///
/// 在库 crate 内部，`t!` 可以在子模块中正常工作（例如
/// `println!("{{}}", t!("key"))`），但此函数故意避免在内部使用
/// `t!`：这样做会重新进入过程宏展开链
///（`t!` → `_rust_i18n_t!` → `rust_i18n::_tr!`），这可能会阻塞同一编译单元中
/// 其他 `t!` 调用点的早期阶段解析。
/// 直接调用 `_rust_i18n_try_translate` 效果相同，且可以避免该风险。
///
/// 如果键没有对应的翻译，则返回该键本身。
pub fn t_str(key: &str) -> String {
    let locale = &rust_i18n::locale();
    _rust_i18n_try_translate(locale, key)
        .unwrap_or(std::borrow::Cow::Borrowed(key))
        .into_owned()
}

/// Embedded Chinese ChangeLog. Other locales fall back to this text until
/// a matching file is added.
/// 嵌入的中文 ChangeLog。在新增对应语言文件前，其他 locale 回退到该文本。
const CHANGELOG_ZH_CN: &str = include_str!("../../../docs/changelog/zh-CN.md");

/// One parsed ChangeLog version block.
/// 一个解析出的 ChangeLog 版本块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogVersion {
    /// Version identifier (e.g. `0.7.0`).
    /// 版本标识（例如 `0.7.0`）。
    pub version: String,
    /// Optional release date in `YYYY-MM-DD` format.
    /// 可选发布日期，格式为 `YYYY-MM-DD`。
    pub date: Option<String>,
    /// The version's body text, without the version heading.
    /// 该版本的正文文本，不含版本标题。
    pub body: String,
}

/// Return the full embedded ChangeLog text for the current locale.
/// 返回当前 locale 对应的完整嵌入 ChangeLog 文本。
pub fn changelog_all() -> &'static str {
    // Only Chinese exists today; every locale uses the same embedded text.
    // 当前只有中文；所有 locale 使用同一嵌入文本。
    CHANGELOG_ZH_CN
}

/// Parse and return every version block in the embedded ChangeLog, in file
/// order.
/// 按文件顺序解析并返回嵌入 ChangeLog 中的每个版本块。
pub fn changelog_versions() -> Vec<ChangelogVersion> {
    parse_changelog(CHANGELOG_ZH_CN)
}

/// Return the body of the requested version, or `None` when the version is
/// not present.
/// 返回指定版本的正文；版本不存在时返回 `None`。
pub fn changelog_version(version: &str) -> Option<String> {
    parse_changelog(CHANGELOG_ZH_CN)
        .into_iter()
        .find(|entry| entry.version == version)
        .map(|entry| entry.body)
}

/// Parse `## [version]`-style headings from `text` into version blocks.
/// 把 `text` 中 `## [版本]` 形式的标题解析为版本块。
fn parse_changelog(text: &str) -> Vec<ChangelogVersion> {
    let mut versions = Vec::new();
    let mut current: Option<ChangelogVersion> = None;

    for line in text.lines() {
        if let Some(entry) = parse_version_heading(line) {
            if let Some(previous) = current.take() {
                versions.push(previous);
            }
            current = Some(entry);
            continue;
        }
        if let Some(entry) = current.as_mut() {
            entry.body.push_str(line);
            entry.body.push('\n');
        }
    }

    if let Some(last) = current {
        versions.push(last);
    }
    versions
}

/// Parse one `## [version]` / `## [version] - date` heading line.
/// 解析一行 `## [版本]` / `## [版本] - 日期` 标题。
fn parse_version_heading(line: &str) -> Option<ChangelogVersion> {
    let rest = line.strip_prefix("## [")?;
    let end = rest.find(']')?;
    let version = rest[..end].trim().to_string();
    if version.is_empty() {
        return None;
    }
    let after = &rest[end + 1..];
    let date = after
        .trim()
        .strip_prefix("- ")
        .map(|date| date.trim().to_string())
        .filter(|date| !date.is_empty());
    Some(ChangelogVersion {
        version,
        date,
        body: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_headings_with_and_without_dates() {
        let entry = parse_version_heading("## [0.7.0] - 2026-08-24").unwrap();
        assert_eq!(entry.version, "0.7.0");
        assert_eq!(entry.date.as_deref(), Some("2026-08-24"));
        assert!(entry.body.is_empty());

        let entry = parse_version_heading("## [0.6.0]").unwrap();
        assert_eq!(entry.version, "0.6.0");
        assert_eq!(entry.date, None);
    }

    #[test]
    fn ignores_non_heading_lines_and_empty_versions() {
        assert!(parse_version_heading("# 更新日志").is_none());
        assert!(parse_version_heading("## []").is_none());
        assert!(parse_version_heading("### Added").is_none());
    }

    #[test]
    fn splits_changelog_into_version_blocks() {
        let text = "# 更新日志

## [0.7.0] - 2026-08-24

### 新增

- A

## [0.6.0]

### 新增

- B
";
        let versions = parse_changelog(text);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, "0.7.0");
        assert!(versions[0].body.contains("- A"));
        assert!(!versions[0].body.contains("## [0.6.0]"));
        assert_eq!(versions[1].version, "0.6.0");
        assert!(versions[1].body.contains("- B"));
    }
}
