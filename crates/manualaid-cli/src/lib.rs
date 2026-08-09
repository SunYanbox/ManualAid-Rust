//! The library behind the `manualaid-cli` executable: mask/restore helpers
//! for privacy snapshots, SKILL discovery and formatting, and a small pager
//! for long console output.
//! `manualaid-cli` 可执行程序背后的库：隐私快照的掩码/还原辅助、SKILL
//! 发现与格式化，以及用于长控制台输出的小型分页器。

pub mod cli;
pub mod commands;
#[doc(hidden)]
pub mod console;
pub mod dir_tree;
mod env;
pub mod pager;
pub mod style;

#[cfg(test)]
pub(crate) mod test_support;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use manualaid_core::error::{CoreError, CoreResult};
use manualaid_core::privacy::{PrivacyMasker, restore_masked_data};
use manualaid_core::skill::{Skill, all_skills, reload_skills};

/// Maximum description characters shown before truncation.
/// 描述截断前最多显示的字符数。
pub const DESCRIPTION_MAX_CHARS: usize = 100;

/// Which skill scopes to include when listing skills.
/// 列出技能时包含的作用域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    /// Both project and global skills.
    /// 项目与全局技能都包含。
    All,
    /// Global skills only.
    /// 仅全局技能。
    Global,
    /// Project skills only.
    /// 仅项目技能。
    Project,
}

/// Resolve CLI input: an existing file is read as UTF-8, an existing
/// non-file path (e.g. a directory) is an error, anything else is treated
/// as literal text.
/// 解析 CLI 输入：已存在的文件按 UTF-8 读取；已存在但不是文件（例如
/// 目录）时报错；其他内容按字面文本处理。
pub fn read_input(input: &str) -> CoreResult<String> {
    let path = Path::new(input);
    if path.is_file() {
        return std::fs::read_to_string(path).map_err(CoreError::from);
    }
    if path.exists() {
        return Err(CoreError::InvalidPath(format!(
            "`{input}` exists but is not a readable file"
        )));
    }
    Ok(input.to_string())
}

/// Mask `input` (text or file path) and return the masked text plus the
/// snapshot mapping (`mask_id → plaintext`) with sorted keys for
/// deterministic JSON output.
/// 掩码 `input`（文本或文件路径），返回掩码文本与按键排序的快照映射
/// （`mask_id → 明文`），保证 JSON 输出确定。
///
/// # Note
/// the snapshot maps mask IDs to plaintext, so it is sensitive data;
/// keep it safe. Restoration cannot work from hash-only data.
/// # 注意
/// 快照将掩码 ID 映射到明文，属于敏感数据，请妥善保管；仅含
/// 哈希的数据无法用于还原。
pub fn mask(masker: &PrivacyMasker, input: &str) -> CoreResult<(String, BTreeMap<String, String>)> {
    mask_with_chars(masker, input).map(|(masked, mapping, _chars)| (masked, mapping))
}

/// Like [`mask`], additionally returning the character count of the resolved
/// input text, so callers can report it without re-reading the input.
/// 同 [`mask`]，额外返回解析后输入文本的字符数，调用方无需重复读取输入。
pub fn mask_with_chars(
    masker: &PrivacyMasker,
    input: &str,
) -> CoreResult<(String, BTreeMap<String, String>, usize)> {
    let text = read_input(input)?;
    let (masked, mapping) = masker.sanitize(&text)?;
    Ok((masked, mapping.into_iter().collect(), text.chars().count()))
}

/// Restore the original text from masked `input` (text or file path) using
/// the snapshot JSON file at `snapshot_path`.
/// 使用 `snapshot_path` 处的快照 JSON，从已掩码的 `input`（文本或文件
/// 路径）还原原文。
pub fn restore(input: &str, snapshot_path: &Path) -> CoreResult<String> {
    restore_with_chars(input, snapshot_path).map(|(restored, _chars)| restored)
}

/// Like [`restore`], additionally returning the character count of the
/// resolved masked input text, so callers can report it without re-reading
/// the input.
/// 同 [`restore`]，额外返回解析后掩码输入文本的字符数，调用方无需重复读取
/// 输入。
pub fn restore_with_chars(input: &str, snapshot_path: &Path) -> CoreResult<(String, usize)> {
    let text = read_input(input)?;
    let snapshot = std::fs::read_to_string(snapshot_path)?;
    let mapping: HashMap<String, String> = serde_json::from_str(&snapshot)
        .map_err(|e| CoreError::Parse(format!("invalid snapshot JSON: {e}")))?;
    Ok((restore_masked_data(&text, &mapping), text.chars().count()))
}

/// Translate `key` and replace `%{name}` placeholders with the given values.
/// 翻译 `key` 并把 `%{name}` 占位符替换为给定值。
///
/// # Description
/// The binary crate cannot use the `t!` macro (it expands inside the crate
/// that invoked `i18n!()`), so templates are fetched with `i18n::t_str` and
/// formatted manually, mirroring the old CLI's approach.
/// # 描述
/// 二进制 crate 不能使用 `t!` 宏（它展开在调用 `i18n!()` 的 crate 内），
/// 因此用 `i18n::t_str` 获取模板后手动格式化，沿用旧版 CLI 的做法。
pub fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
}

/// Format a [`Duration`] as milliseconds with exactly six decimal places,
/// computed from the integer nanosecond count so the value is exact to the
/// nanosecond (1 ns = 0.000001 ms).
/// 将 [`Duration`] 格式化为毫秒并固定保留 6 位小数，由整数纳秒数直接换算，
/// 保证精确到纳秒（1 ns = 0.000001 ms）。
pub fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    let millis = nanos / 1_000_000;
    let fraction = nanos % 1_000_000;
    format!("{millis}.{fraction:06} ms")
}

/// Format a byte count with an automatically chosen 1024-based unit
/// (KB/MB/GB) and exactly three decimal places.
/// 将字节数按 1024 进制自动选择单位（KB/MB/GB）并固定保留 3 位小数。
pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.3} GB", value / GB)
    } else if value >= MB {
        format!("{:.3} MB", value / MB)
    } else {
        format!("{:.3} KB", value / KB)
    }
}

/// Format a "Timings" section from already-localized lines. Returns an empty
/// string for an empty slice; otherwise a headed section with `  - ` prefixed
/// lines, following the styling of the other CLI output sections.
/// 由已本地化的行格式化“耗时”区块。空切片返回空串；否则输出带标题的区块，
/// 每行以 `  - ` 开头，样式与其他 CLI 输出区块一致。
pub fn format_timings(lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let heading = style::header(&t_fmt("cli.output.timings", &[]));
    let body = lines
        .iter()
        .map(|line| format!("  - {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n{heading}\n{body}\n")
}

/// Rescan skills for `project_root` and return all loaded skills.
/// 为 `project_root` 重新扫描技能并返回全部已加载技能。
pub fn load_skills(project_root: &Path) -> CoreResult<Vec<Skill>> {
    reload_skills(project_root)?;
    Ok(all_skills())
}

/// Keep only the skills matching `scope`.
/// 仅保留与 `scope` 匹配的技能。
pub fn filter_skills(skills: Vec<Skill>, scope: SkillScope) -> Vec<Skill> {
    match scope {
        SkillScope::All => skills,
        SkillScope::Global => skills.into_iter().filter(|s| s.is_global).collect(),
        SkillScope::Project => skills.into_iter().filter(|s| !s.is_global).collect(),
    }
}

/// Return the source directory of a skill: the parent of its `skills`
/// directory (e.g. `~/.cc-switch` for `~/.cc-switch/skills/theme-factory`).
/// 返回技能的来源目录：其 `skills` 目录的父目录（例如
/// `~/.cc-switch/skills/theme-factory` 对应 `~/.cc-switch`）。
pub fn skill_source_path(skill: &Skill) -> PathBuf {
    skill
        .path
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| skill.path.clone())
}

/// Format one skill as localized indented lines: unique name, name,
/// truncated description and full description character count.
/// 按本地化缩进行格式化一个技能：唯一名称、名称、截断后的描述与完整
/// 描述字符数。
pub fn format_skill(skill: &Skill) -> String {
    let description = truncate_description(&skill.description, DESCRIPTION_MAX_CHARS);
    [
        style::bold(&t_fmt(
            "cli.skill.unique_name",
            &[("unique_name", &skill.unique_name)],
        )),
        t_fmt("cli.skill.name", &[("name", &skill.name)]),
        t_fmt("cli.skill.desc", &[("desc", &description)]),
        style::muted(&t_fmt(
            "cli.skill.desc_chars_total",
            &[("chars", &skill.description.chars().count().to_string())],
        )),
    ]
    .join("\n")
}

/// Format skills grouped by source directory: one `- <source>` header line
/// followed by each skill's indented lines.
/// 按来源目录分组格式化技能：每个 `- <来源>` 标题行后跟各技能的缩进行。
pub fn format_skill_list(skills: &[Skill]) -> String {
    let mut lines = Vec::new();
    let mut current_source: Option<PathBuf> = None;
    for skill in skills {
        let source = skill_source_path(skill);
        if current_source.as_ref() != Some(&source) {
            if current_source.is_some() {
                lines.push(String::new());
            }
            lines.push(style::accent(&t_fmt(
                "cli.skill.source",
                &[("path", &source.display().to_string())],
            )));
            current_source = Some(source);
        }
        lines.push(format_skill(skill));
    }
    lines.join("\n")
}

/// Format the startup message; styled terminals get a green success line
/// surrounded by blank lines.
/// 格式化启动消息；样式终端输出带上下空行的绿色成功行。
pub fn format_default_output(message: &str) -> String {
    if style::is_enabled() {
        format!("\n{}\n", style::success(message))
    } else {
        format!("{message}\n")
    }
}

/// Format the mask output: the masked text and the pretty snapshot JSON as
/// two headed sections.
/// 格式化掩码输出：掩码文本与 pretty 快照 JSON 两个带标题的区块。
pub fn format_mask_output(masked: &str, snapshot_json: &str) -> String {
    let masked_heading = style::header(&t_fmt("cli.output.masked_text", &[]));
    let snapshot_heading = style::header(&t_fmt("cli.output.snapshot_json", &[]));
    format!("\n{masked_heading}\n{masked}\n\n{snapshot_heading}\n{snapshot_json}\n")
}

/// Format the restore output; styled terminals get a heading and a green
/// restored text, otherwise the plain text is returned unchanged for piping.
/// 格式化还原输出；样式终端带标题并输出绿色还原文本，否则原样返回纯文本
/// 以便管道使用。
pub fn format_restore_output(restored: &str) -> String {
    if restored.is_empty() {
        return String::new();
    }
    if style::is_enabled() {
        let heading = style::header(&t_fmt("cli.output.restored_text", &[]));
        format!("\n{heading}\n{}\n", style::success(restored))
    } else {
        format!("{restored}\n")
    }
}

/// Format the skill listing; styled terminals get a count heading and a
/// blank line before the list.
/// 格式化技能列表；样式终端带数量标题和列表前的空行。
pub fn format_skill_output(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let list = format_skill_list(skills);
    if style::is_enabled() {
        let count = skills.len().to_string();
        let heading = style::header(&t_fmt("cli.output.skills", &[("count", &count)]));
        format!("\n{heading}\n\n{list}\n")
    } else {
        format!("{list}\n")
    }
}

/// Format an error message; styled terminals get a bold red "Error: ..."
/// line with a trailing blank line.
/// 格式化错误消息；样式终端输出加粗红色的“错误：...”并追加空行。
pub fn format_error_output(error: &str) -> String {
    if style::is_enabled() {
        let message = t_fmt("cli.error.prefix", &[("message", error)]);
        format!("{}\n", style::error(&message))
    } else {
        format!("{error}\n")
    }
}

/// Truncate `description` to `max_chars` characters, appending `…` when it
/// is longer.
/// 将 `description` 截断到 `max_chars` 个字符，超长时追加 `…`。
pub fn truncate_description(description: &str, max_chars: usize) -> String {
    if description.chars().count() <= max_chars {
        description.to_string()
    } else {
        let truncated: String = description.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}
