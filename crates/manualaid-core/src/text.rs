//! Shared text-formatting helpers.
//! 共享的文本格式化辅助。

/// Translate `key` and replace `%{name}` placeholders with the given values.
/// 翻译 `key` 并把 `%{name}` 占位符替换为给定值。
///
/// # Description
/// Mirrors the helpers currently duplicated in the CLI and WebSocket crates;
/// those copies are meant to be folded into this one later. Placeholders that
/// are absent from `args` are left untouched, so a missing argument shows up
/// in the output instead of being silently dropped, and a shorter name never
/// matches inside a longer one (`%{name}` does not touch `%{unique_name}`).
/// # 描述
/// 与 CLI 和 WebSocket crate 中现存的同名辅助函数一致；这些副本计划在后续
/// 统一到此函数。未在 `args` 中提供的占位符保持原样，使缺失的参数出现在
/// 输出中而非被静默丢弃；较短的名称也不会匹配较长名称的内部
///（`%{name}` 不会影响 `%{unique_name}`）。
pub fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
