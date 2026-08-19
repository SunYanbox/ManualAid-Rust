/// Frontmatter fields extracted from a `SKILL.md` file.
/// 从 `SKILL.md` 文件提取的 frontmatter 字段。
pub(super) struct Frontmatter {
    /// `name:` value when present and non-empty; the caller falls back to
    /// the folder name when missing.
    /// `name:` 存在且非空时的值；缺失时由调用方回退为目录名。
    pub(super) name: Option<String>,
    /// `description:` value; may be empty, the caller decides to skip.
    /// `description:` 的值；可能为空，由调用方决定跳过。
    pub(super) description: String,
    /// Markdown after the closing `---` delimiter.
    /// 结束 `---` 分隔符之后的 Markdown 正文。
    pub(super) body: String,
}

/// Parse YAML-style frontmatter and body from a `SKILL.md` content.
///
/// Returns `Some` whenever a leading `---` block exists, with `name` as
/// `None` when the field is missing or empty; `description` may be empty.
/// Returns `None` when no leading `---` block exists.
/// 从 `SKILL.md` 内容解析 YAML 风格 frontmatter 与正文。
///
/// 只要存在前导 `---` 块即返回 `Some`，`name` 字段缺失或为空时为 `None`；
/// `description` 可能为空。无前导 `---` 块时返回 `None`。
pub(super) fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_opener = &trimmed[3..];
    let end = after_opener.find("\n---")?;
    let frontmatter = after_opener[..end].trim();

    let after_closing = &after_opener[end + 4..];
    let body = match after_closing.find('\n') {
        Some(nl) => after_closing[nl + 1..].to_string(),
        None => String::new(),
    };

    let mut name = None;
    let mut description = None;

    #[derive(Clone, Copy)]
    enum BlockMode {
        /// YAML `>` folded block — newlines fold to spaces, blank lines
        /// become paragraph breaks.
        /// YAML `>` 折叠块——换行折叠为空格，空行变为段落分隔。
        Folded,
        /// YAML `|` literal block — newlines preserved.
        /// YAML `|` 字面量块——保留换行符。
        Literal,
    }

    let mut collect: Option<(String, BlockMode)> = None;
    let mut blank_line = false;

    for raw_line in frontmatter.lines() {
        if let Some((ref mut acc, mode)) = collect {
            if raw_line.starts_with(' ') || raw_line.is_empty() {
                let content = raw_line.trim();
                if content.is_empty() {
                    match mode {
                        BlockMode::Folded => blank_line = true,
                        BlockMode::Literal => acc.push('\n'),
                    }
                    continue;
                }
                if blank_line {
                    acc.push('\n');
                    blank_line = false;
                } else if !acc.is_empty() {
                    match mode {
                        BlockMode::Folded => acc.push(' '),
                        BlockMode::Literal => acc.push('\n'),
                    }
                }
                acc.push_str(content);
                continue;
            }

            description = Some(std::mem::take(acc));
            collect = None;
        }

        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key == "name" && !value.is_empty() {
                name = Some(value.to_string());
            }
            if key == "description" {
                if value == ">" || value == ">-" || value == ">+" {
                    collect = Some((String::new(), BlockMode::Folded));
                } else if value == "|" || value == "|-" || value == "|+" {
                    collect = Some((String::new(), BlockMode::Literal));
                } else if value.is_empty() {
                    collect = Some((String::new(), BlockMode::Folded));
                } else {
                    description = Some(value.to_string());
                }
            }
        }
    }

    if let Some((acc, _)) = collect
        && !acc.is_empty()
        && description.is_none()
    {
        description = Some(acc);
    }

    Some(Frontmatter {
        name,
        description: description.unwrap_or_default(),
        body,
    })
}
