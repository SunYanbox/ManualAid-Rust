//! System prompt building: renders the enabled-tools section with the
//! active wire format, the workspace context and the enabled-skills list,
//! plus the bracket-wrapped result text copied back to the clipboard.
//! 系统提示词构建：按当前线格式渲染已启用工具区块、工作区上下文与
//! 已启用技能列表；并提供复制回剪贴板的方括号包裹结果文本。

use std::fmt::Write;
use std::path::{Path, PathBuf};

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::{Skill, exposed_name_map};
use manualaid_core::tools::{ToolKind, ToolResult, UserAction};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::context;

/// Build the full `<system_prompt>` text for `config`.
/// 为 `config` 构建完整的 `<system_prompt>` 文本。
pub fn build_system_prompt(
    config: &Config,
    workspace_root: &Path,
    registry: &FormatRegistry,
    skills: &[Skill],
    context_files: &[PathBuf],
) -> String {
    // With no enabled skills the skill tool is useless, so the prompt is
    // generated as if the skill switch were off: no skill rules and no
    // skill tool in the available-tools list.
    // 没有已启用技能时 Skill 工具没有用处，因此按 SKILL 开关关闭来生成
    // 提示词：不输出技能规则，也不在可用工具列表中列出 Skill 工具。
    let skill_active = config.skill && skills.iter().any(|skill| skill.is_enabled);
    let effective = Config {
        skill: skill_active,
        ..config.clone()
    };
    let tools_list = render_tools_list(&effective, registry);
    let format_desc = tool_calling_format_description(registry);

    let mut out = String::new();
    out.push_str("<system_prompt>\n");
    out.push_str(&i18n::t_str("prompt.system.capabilities"));
    out.push('\n');
    out.push_str(&i18n::t_str("prompt.system.system-reminder-note"));
    out.push('\n');
    out.push_str(&i18n::t_str("prompt.system.user-action-note"));
    out.push('\n');
    out.push_str(&i18n::t_str("prompt.system.intent-output-rule"));
    out.push('\n');
    out.push_str(&t_fmt(
        "prompt.system.common-rules",
        &[
            ("current_tool_format_description", &format_desc),
            ("tools_list", &tools_list),
        ],
    ));
    if skill_active {
        out.push('\n');
        out.push_str(&i18n::t_str("prompt.system.skill-rule"));
    }
    push_section(&mut out, &platform_notes_text());
    out.push('\n');

    let workspace_info = workspace_info_text(workspace_root);
    let context_files_text = if config.context_auto_load {
        context::render_context_files(context_files)
    } else {
        String::new()
    };
    let skills_list = if skill_active {
        skills_list_text(skills)
    } else {
        String::new()
    };
    // Unfinished TODO lists are injected only while the todo_write tool is
    // enabled: the context is useless when the model cannot refresh it.
    // 仅在 todo_write 工具启用时注入未完成 TODO 列表：模型无法刷新时该上下文
    // 没有意义。
    let todo_context = if config.todo_write {
        manualaid_core::todo::unfinished_todos(workspace_root)
            .map(|body| format!("<unfinished_todos>\n{body}\n</unfinished_todos>"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    out.push_str(&t_fmt(
        "prompt.system.dynamic-context",
        &[
            ("workspace_info", &workspace_info),
            ("skills_list", &skills_list),
            ("todo_context", &todo_context),
        ],
    ));
    out.push_str("\n</system_prompt>");

    // Context files live outside the system prompt so they are not treated
    // as system-level instructions; the optional block keeps the output
    // ending at </system_prompt> when no context file can be rendered.
    // 上下文文件位于 system prompt 之外，避免被当作系统级指令；无可渲染
    // 上下文文件时不追加该块，使输出仍以 </system_prompt> 结尾。
    if !context_files_text.is_empty() {
        out.push_str("\n\n");
        out.push_str(&render_context_reminder(&context_files_text));
    }
    out
}

/// Render the full `<system-reminder>` block for the given context-file
/// text. Shared by the system-prompt builder and the copy-context menu.
/// 为给定上下文文件文本渲染完整的 `<system-reminder>` 块。系统提示词构建器
/// 与复制上下文菜单共用。
pub fn render_context_reminder(context_files_text: &str) -> String {
    let mut out = String::new();
    out.push_str("<system-reminder>\n");
    out.push_str(&i18n::t_str("prompt.system.context-files-reminder"));
    // The locale value already ends with a newline; this second newline
    // turns it into the blank line between the reminder and the files.
    // locale 值已自带换行；此处再补一个换行，使引导语与文件之间空一行。
    out.push('\n');
    out.push_str(context_files_text);
    out.push_str("</system-reminder>");
    out
}

/// Render the current tool-calling-format description: the localized
/// format preamble, a single default tool call template (`read`) and the
/// plain-text formatting notes. Shared by the system prompt builder and the
/// copy-prompt submenu.
/// 渲染当前工具调用格式说明：本地化的格式前言、单个默认工具调用模板
/// （`read`）与纯文本格式说明。系统提示词构建器与复制提示词二级菜单共用。
pub fn tool_calling_format_description(registry: &FormatRegistry) -> String {
    format!(
        "{}\n```func_calls\n{}\n```\n{}\n",
        i18n::t_str("cli.prompt.format_desc"),
        registry
            .render_tool_call_template(&manualaid_core::parser::ToolTemplate::from_kind(
                ToolKind::Read,
            ))
            .unwrap_or_default(),
        i18n::t_str("cli.prompt.func_calls_notes")
    )
}

/// Render the `<available-tools>`-style section for the enabled tools,
/// with localized descriptions and call templates in the active format.
/// 为已启用工具渲染 `<available-tools>` 风格的区块：本地化描述与当前
/// 格式的调用模板。
pub fn render_tools_list(config: &Config, registry: &FormatRegistry) -> String {
    let mut out = String::new();
    for tool in manualaid_core::tools::all_tools() {
        if !is_enabled(config, tool) {
            continue;
        }
        let _ = write!(out, "## {}\n\n{}\n\n", tool.name(), tool.description());

        let params = tool.parameters();
        if !params.is_empty() {
            out.push_str("**Parameters:**\n");
            for param in &params {
                let requirement = if param.required {
                    "required"
                } else {
                    "optional"
                };
                let _ = writeln!(
                    out,
                    "- `{}` (`{}`) ({requirement}): {}",
                    param.name,
                    param.kind,
                    param.description()
                );
            }
            out.push('\n');
        }

        out.push_str("**Call template:**\n\n```func_calls\n");
        if let Ok(template) = registry
            .render_tool_call_template(&manualaid_core::parser::ToolTemplate::from_kind(*tool))
        {
            out.push_str(&template);
        }
        out.push_str("\n```\n\n");
    }

    // MCP tools follow the built-ins: they are enabled per server rather than
    // per tool, so the `[tools]` switches do not apply to them.
    // MCP 工具列在内置工具之后：它们按服务器而非按工具启用，因此
    // `[tools]` 开关对其不适用。
    for tool in manualaid_core::mcp::enabled_tools() {
        let _ = write!(out, "## {}\n\n{}\n\n", tool.exposed_name, tool.description);

        if !tool.params.is_empty() {
            out.push_str("**Parameters:**\n");
            for param in &tool.params {
                let requirement = if param.required {
                    "required"
                } else {
                    "optional"
                };
                let _ = writeln!(
                    out,
                    "- `{}` (`{}`) ({requirement}): {}",
                    param.name, param.kind, param.description
                );
            }
            out.push('\n');
        }

        out.push_str("**Call template:**\n\n```func_calls\n");
        if let Ok(template) = registry
            .render_tool_call_template(&manualaid_core::parser::ToolTemplate::from_mcp(&tool))
        {
            out.push_str(&template);
        }
        out.push_str("\n```\n\n");
    }
    out
}

/// Render the enabled-tools list wrapped in a single `<system-reminder>`
/// block for copying to the clipboard. The system prompt keeps using
/// [`render_tools_list`] inside its own `<available-tools>` section, so the
/// two surfaces can evolve independently.
/// 将已启用工具列表包裹在单个 `<system-reminder>` 块中，供复制到剪贴板。
/// 系统提示词仍在自身的 `<available-tools>` 区块内使用
/// [`render_tools_list`]，使两种用途可独立演进。
pub fn render_tools_list_reminder(config: &Config, registry: &FormatRegistry) -> String {
    let inner = render_tools_list(config, registry);
    format!(
        "<system-reminder>\n{}\n</system-reminder>",
        inner.trim_end()
    )
}

/// Render the enabled-skills list wrapped in a single `<system-reminder>`
/// block for copying to the clipboard. When no skill is enabled the block
/// still exists but carries the localized empty placeholder so the copied
/// text never looks like a silent failure.
/// 将已启用技能列表包裹在单个 `<system-reminder>` 块中，供复制到剪贴板。
/// 无已启用技能时该块仍存在，但内部为本地化占位文案，使复制结果不会
/// 看起来像静默失败。
pub fn render_skills_list(skills: &[Skill]) -> String {
    let inner = skills_list_text(skills);
    let body = if inner.is_empty() {
        i18n::t_str("prompt.copy.skills-empty")
    } else {
        inner.trim_end().to_string()
    };
    format!("<system-reminder>\n{body}\n</system-reminder>")
}

/// Whether `tool` is enabled by `config`.
/// `tool` 是否被 `config` 启用。
fn is_enabled(config: &Config, tool: &ToolKind) -> bool {
    match tool {
        ToolKind::Shell => config.shell,
        ToolKind::Read => config.read,
        ToolKind::Edit => config.edit,
        ToolKind::Write => config.write,
        ToolKind::Skill => config.skill,
        ToolKind::TodoWrite => config.todo_write,
    }
}

/// Windows-specific guidance rendered only when the prompt is built on
/// Windows; empty on other platforms.
/// 仅在 Windows 平台上构建提示词时渲染的 Windows 专属提示；其他平台为空。
fn platform_notes_text() -> String {
    // Detect the Microsoft CoreUtils extension by its well-known install
    // path; the probe is compiled only on Windows. A `Path::is_file()` check
    // is cheaper and more reliable than spawning a command, and the prompt
    // build is a synchronous path.
    // 通过官方安装路径探测微软 CoreUtils 扩展；该探测仅在 Windows 下编译。
    // `Path::is_file()` 检查比 spawn 命令更廉价可靠，且提示词构建是同步路径。
    #[cfg(windows)]
    let coreutils_installed = Path::new("C:/Program Files/coreutils/coreutils.exe").is_file();
    #[cfg(not(windows))]
    let coreutils_installed = false;
    platform_notes_text_for(cfg!(windows), coreutils_installed)
}

/// The Windows guidance for an explicit platform decision and CoreUtils
/// availability, split out so tests can exercise every branch on any host.
/// 按显式平台决策与 CoreUtils 可用性返回 Windows 提示文本；拆出独立函数
/// 以便测试在任意宿主平台上显式覆盖所有分支。
fn platform_notes_text_for(is_windows: bool, coreutils_installed: bool) -> String {
    if !is_windows {
        return String::new();
    }
    let notes = i18n::t_str("prompt.system.platform-notes");
    if !coreutils_installed {
        return notes;
    }
    // Insert the CoreUtils paragraph just before the closing tag so both
    // parts share one <platform-notes> block; without CoreUtils the note
    // never appears.
    // 在闭合标签前插入 CoreUtils 段落，使两部分共处于同一个
    // <platform-notes> 块中；未安装 CoreUtils 时不出现该备注。
    let coreutils = i18n::t_str("prompt.system.coreutils-notes");
    notes.replacen(
        "</platform-notes>",
        &format!("\n\n{coreutils}\n</platform-notes>"),
        1,
    )
}

/// Append a blank line followed by `section` to `out` when `section` is
/// non-empty; a no-op otherwise.
/// 当 `section` 非空时，在 `out` 末尾先补一个空行再追加 `section`；
/// `section` 为空时不做任何事。
fn push_section(out: &mut String, section: &str) {
    if !section.is_empty() {
        out.push('\n');
        out.push_str(section);
    }
}

/// The `<dynamic-context>` workspace section: root path, shell, git info, and directory listing.
/// `<dynamic-context>` 的工作区部分：根路径、Shell、git 信息与目录列表。
fn workspace_info_text(workspace_root: &Path) -> String {
    let shell = manualaid_core::shell::detected_shell();
    let git_info = git_info_text(workspace_root);
    let dir_list = directory_listing_text(workspace_root);
    let mut result = format!(
        "<workspace_root>\n{}\n</workspace_root>\n<shell_environment>\n{shell}\n</shell_environment>\n",
        workspace_root.display()
    );
    if !git_info.is_empty() {
        result.push_str("<git_information>\n");
        // The git block opens with a localized snapshot note.
        // git 块开头附本地化快照备注。
        result.push_str(&i18n::t_str("prompt.system.git-status-note"));
        result.push_str("\n\n");
        result.push_str(&git_info);
        result.push_str("</git_information>\n");
    }
    if !dir_list.is_empty() {
        result.push_str("<directory_listing>\n");
        // The directory listing block opens with a localized snapshot note.
        // 目录列表块开头附本地化快照备注。
        result.push_str(&i18n::t_str("prompt.system.directory-listing-note"));
        result.push_str("\n\n");
        result.push_str(&dir_list);
        result.push_str("\n</directory_listing>\n");
    }
    result
}

/// Capture `git status` and `git log --oneline -5` output if git is available.
/// 若 git 可用，捕获 `git status` 与 `git log --oneline -5` 输出。
fn git_info_text(workspace_root: &Path) -> String {
    use std::process::Command;
    let mut output = String::new();

    // Check if git is available and the directory is a git repository.
    // 检查 git 是否可用且目录是否为 git 仓库。
    let git_check = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(workspace_root)
        .output();
    if let Ok(check) = git_check {
        if !check.status.success() {
            return String::new();
        }
    } else {
        return String::new();
    }

    // Git status
    // git 状态
    let status = Command::new("git")
        .arg("status")
        .current_dir(workspace_root)
        .output();
    if let Ok(out) = status
        && out.status.success()
        && let Ok(text) = String::from_utf8(out.stdout)
    {
        output.push_str(&text);
        if !text.ends_with('\n') {
            output.push('\n');
        }
    }

    // Git log (oneline, last 5 commits)
    // git 日志（单行，最近5条）
    let log = Command::new("git")
        .args(["log", "--oneline", "-5"])
        .current_dir(workspace_root)
        .output();
    if let Ok(out) = log
        && out.status.success()
        && let Ok(text) = String::from_utf8(out.stdout)
    {
        output.push_str(&text);
        if !text.ends_with('\n') {
            output.push('\n');
        }
    }

    output
}

/// List files and directories under the workspace root, similar to `ls -la`.
/// 列出工作区根目录下的文件和文件夹，类似 `ls -la`。
fn directory_listing_text(workspace_root: &Path) -> String {
    use std::fs;
    let mut lines = Vec::new();
    if let Ok(entries) = fs::read_dir(workspace_root) {
        let mut items: Vec<(String, bool)> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            items.push((name, is_dir));
        }
        items.sort_by_key(|(name, _)| name.to_lowercase());
        for (name, is_dir) in items {
            let marker = if is_dir { "<DIR>" } else { "     " };
            lines.push(format!("{marker}  {name}"));
        }
    }
    lines.join("\n")
}

/// The `<dynamic-context>` skills section from the enabled skills.
///
/// Each entry shows the exposed name — the shortest name that identifies the
/// skill among the enabled set — plus its description, so the agent can pass
/// the shown token verbatim to the Skill tool. The plain `name` and the full
/// stable unique name are redundant in the system prompt.
/// 由已启用技能生成的 `<dynamic-context>` 技能部分。
///
/// 每条目展示暴露名——在已启用集合中唯一识别该技能的最短名称——及描述，
/// 代理可将所示记号原样传给 Skill 工具。裸 `name` 与完整稳定唯一名在
/// 系统提示中冗余。
fn skills_list_text(skills: &[Skill]) -> String {
    let enabled: Vec<Skill> = skills.iter().filter(|s| s.is_enabled).cloned().collect();
    let exposed = exposed_name_map(&enabled);
    let mut out = String::new();
    for skill in &enabled {
        let name = &exposed[skill.unique_name.as_str()];
        let _ = writeln!(out, "- {name}: {}", skill.description);
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("<available_skills>\n{out}</available_skills>\n")
    }
}

/// Minimum characters retained for any individual tool result that is
/// subject to truncation. Results no longer than this are never shortened.
/// 任何被截断的单个工具结果最少保留的字符数。不超过该值的结果永不被缩短。
const MIN_KEEP_CHARS: usize = 1000;

/// Lines of already-shown context repeated when a resume hint points back
/// into the persisted copy, so the agent rejoins the text a little before
/// the cut rather than exactly at it.
/// 续读指引指回暂存副本时重复的、已显示过的上下文行数，使代理在截断点
/// 稍前处重新接入文本，而非恰好从截断点开始。
const CONTEXT_LINES_BEFORE_CUT: usize = 2;

/// A tool result split into its XML wrapper parts and the variable content
/// for easy size accounting and truncation.
/// 将工具结果拆分为 XML 包裹部分与可变内容部分，便于尺寸计算与截断。
struct ResultPart {
    header: String,
    content: String,
    footer: String,
    tool_name: String,
    params_summary: String,
    user_action: Option<UserAction>,
}

/// Render one round's execution results as XML-wrapped text for pasting
/// back into an external LLM chat. The character limit applies to the sum
/// of the tool outputs only (the XML wrappers are not counted). If that sum
/// exceeds `max_result_chars`, every result longer than `MIN_KEEP_CHARS`
/// is truncated proportionally to its original size, keeping at least
/// `MIN_KEEP_CHARS` characters; shorter results stay whole. Each
/// truncated result carries a notice with its removed character count, and
/// a round-level warning is appended at the end so both the user and the
/// LLM know content was omitted. That warning is followed by one list entry
/// per result pointing into the persisted copy whenever the outputs were
/// successfully saved, so the omitted lines can be read back with the `read`
/// tool's `offset`/`limit` parameters.
/// 将一轮执行结果渲染为 XML 包裹文本，供回贴到外部 LLM 聊天。字符限制
/// 只作用于各工具输出之和（不计 XML 包裹部分）。当该和超过
/// `max_result_chars` 时，每个超过 `MIN_KEEP_CHARS` 字符的结果按原始
/// 大小比例截断，且至少保留 `MIN_KEEP_CHARS` 字符；较短的结果保持
/// 完整。每个被截断的结果附带一条含被截断字符数的标注，末尾追加轮次
/// 警告，让用户与 LLM 都能知晓内容已被省略。暂存成功时，警告之后为每个
/// 结果附一条指向暂存副本的清单条目，使被省略的行可用 `read` 工具的
/// `offset`/`limit` 参数读回。
pub fn format_results(
    results: &[ToolResult],
    max_result_chars: usize,
    workspace_root: &Path,
) -> String {
    if results.is_empty() {
        return String::new();
    }

    let parts: Vec<ResultPart> = results
        .iter()
        .map(|result| {
            ResultPart {
                header: result_header(result),
                // Keep the tool output verbatim: trimming would drop trailing
                // spaces that can be significant in read slices or code blocks.
                // 保留工具输出原文：trim 会丢失 read 切片或代码块中可能有意义的尾部空格。
                content: result.output.to_string(),
                footer: result_footer(result),
                tool_name: result.tool_name.clone(),
                params_summary: result.params_summary.clone(),
                user_action: result.user_action.clone(),
            }
        })
        .collect();

    let separator = "\n\n";
    let content_total: usize = parts.iter().map(|p| p.content.chars().count()).sum();

    if content_total <= max_result_chars {
        return parts
            .iter()
            .map(|p| format!("{}{}{}", p.header, p.content, p.footer))
            .collect::<Vec<_>>()
            .join(separator);
    }

    // Persist the complete un-truncated tool outputs to
    // `<workspace_root>/.ManualAid/temp/<sha256>.md` so the model can still
    // inspect the omitted content when needed. Failures are silent: the
    // truncated text remains usable and the copy flow is not interrupted.
    // 将未截断的完整工具输出写入 `<workspace_root>/.ManualAid/temp/<sha256>.md`，
    // 使模型在需要时仍可查看被省略的内容。写文件失败时静默降级：截断文本
    // 保持可用，复制流程不被中断。
    let persisted = persist_full_output(&parts, workspace_root);

    // `Some(line)` marks the first temp-file line the agent has not seen, so
    // the list entry can point them back at it; `None` means the result is
    // shown whole and needs no pointer.
    // `Some(line)` 表示代理尚未看到的首个临时文件行，供清单条目指回该处；
    // `None` 表示结果完整显示、无需指引。
    let mut hidden: Vec<Option<usize>> = vec![None; parts.len()];

    // Short results are never shortened and do not take part in the
    // proportional split; they still occupy their full length in the budget.
    // 短结果永不被缩短、不参与比例分配，但仍按完整长度占用预算。
    let eligible: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.content.chars().count() > MIN_KEEP_CHARS)
        .map(|(i, _)| i)
        .collect();

    // Nothing can be shortened: drop whole results from the end until the
    // remaining content fits, then append the round warning.
    // 没有可缩短的结果：从末尾整块丢弃，直到剩余内容放得下，再追加警告。
    if eligible.is_empty() {
        let mut shown = 0usize;
        let mut used = 0usize;
        for p in &parts {
            let content_len = p.content.chars().count();
            if used + content_len > max_result_chars {
                break;
            }
            used += content_len;
            shown += 1;
        }
        // A dropped result was never shown at all, so the point to resume
        // from is its own first line.
        // 被丢弃的结果完全未显示，因此从它自己的首行续读。
        if let Some((_, ranges)) = &persisted {
            for (slot, range) in hidden.iter_mut().zip(ranges).skip(shown) {
                *slot = Some(range.start);
            }
        }

        let mut result = String::new();
        for p in parts.iter().take(shown) {
            let block = format!("{}{}{}", p.header, p.content, p.footer);
            if result.is_empty() {
                result.push_str(&block);
            } else {
                result.push_str(separator);
                result.push_str(&block);
            }
        }
        result.push_str(&round_tail(
            &parts,
            &hidden,
            persisted.as_ref(),
            max_result_chars,
            content_total,
        ));
        return result;
    }

    let eligible_orig_total: usize = eligible
        .iter()
        .map(|&i| parts[i].content.chars().count())
        .sum();
    let ineligible_total: usize = (0..parts.len())
        .filter(|i| !eligible.contains(i))
        .map(|i| parts[i].content.chars().count())
        .sum();
    let budget_for_eligible = max_result_chars.saturating_sub(ineligible_total);

    let mut allocs: Vec<usize> = vec![0; parts.len()];
    let mut raw_sum = 0usize;
    for &i in &eligible {
        let orig = parts[i].content.chars().count();
        let raw = ((budget_for_eligible as f64) * (orig as f64) / (eligible_orig_total as f64))
            .floor() as usize;
        let alloc = raw.max(MIN_KEEP_CHARS);
        allocs[i] = alloc;
        raw_sum += alloc;
    }

    // The minimum-keep floor can push the sum over the budget; take the
    // excess back from the largest allocations, never below the floor.
    // 保底下限可能使分配总和超出预算；从最大的分配开始回扣，但不低于下限。
    if raw_sum > budget_for_eligible {
        let overshoot = raw_sum - budget_for_eligible;
        let mut sorted: Vec<(usize, usize)> = eligible.iter().map(|&i| (allocs[i], i)).collect();
        sorted.sort_by_key(|(a, _)| std::cmp::Reverse(*a));
        let mut remaining = overshoot;
        for &(_alloc, idx) in &sorted {
            let can_reduce = allocs[idx].saturating_sub(MIN_KEEP_CHARS);
            let reduce = remaining.min(can_reduce);
            allocs[idx] -= reduce;
            remaining -= reduce;
            if remaining == 0 {
                break;
            }
        }
    }

    if let Some((_, ranges)) = &persisted {
        for &i in &eligible {
            hidden[i] = Some(first_hidden_line(&parts[i], &ranges[i], allocs[i]));
        }
    }

    let mut blocks: Vec<String> = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        if eligible.contains(&i) {
            let orig = part.content.chars().count();
            let alloc = allocs[i];
            let notice = i18n::t_str("truncated_tool_result")
                .replace("%{original}", &orig.to_string())
                .replace("%{removed}", &(orig - alloc).to_string());
            let truncated: String = part.content.chars().take(alloc).collect();
            blocks.push(format!(
                "{}{}\n{}{}",
                part.header, truncated, notice, part.footer
            ));
        } else {
            blocks.push(format!("{}{}{}", part.header, part.content, part.footer));
        }
    }

    let mut result = blocks.join(separator);
    result.push_str(&round_tail(
        &parts,
        &hidden,
        persisted.as_ref(),
        max_result_chars,
        content_total,
    ));
    result
}

/// Render the round-level truncation warning, followed by one list entry per
/// result when the outputs were persisted. Entries let the agent resume from
/// the temp file with the `read` tool; see [`persisted_entry`].
/// 渲染轮次截断警告；暂存成功时再附上每个结果一条的清单条目。条目使代理
/// 可用 `read` 工具从临时文件续读；见 [`persisted_entry`]。
fn round_tail(
    parts: &[ResultPart],
    hidden: &[Option<usize>],
    persisted: Option<&(PathBuf, Vec<PartRange>)>,
    max_result_chars: usize,
    content_total: usize,
) -> String {
    let mut tail = format!(
        "\n\n{}",
        i18n::t_str("truncated_round_warning")
            .replace("%{max_chars}", &max_result_chars.to_string())
            .replace("%{total_chars}", &content_total.to_string())
    );

    if let Some((temp_path, ranges)) = persisted {
        let tools: Vec<String> = parts
            .iter()
            .zip(ranges)
            .enumerate()
            .map(|(i, (part, range))| persisted_entry(&persisted_label(part), range, hidden[i]))
            .collect();
        tail.push_str(
            &i18n::t_str("truncated_persisted_notice")
                .replace("%{temp_path}", &temp_path.to_string_lossy())
                .replace("%{tools}", &tools.join("\n")),
        );
    }

    tail
}

/// Label identifying one result inside the persisted-output list.
/// 在暂存输出清单中标识单个结果的标签。
fn persisted_label(part: &ResultPart) -> String {
    if let Some(action) = &part.user_action {
        format!("{} ({})", action.kind.as_str(), action.label)
    } else if part.params_summary.is_empty() {
        part.tool_name.clone()
    } else {
        format!("{} ({})", part.tool_name, part.params_summary)
    }
}

/// Render one persisted-output list entry. A result shown whole only needs its
/// start line; a result the agent has not fully seen gets an `offset`/`limit`
/// pair spanning from two lines before the first unseen line through the end
/// of that result's block, so the resumed read repeats a little context and
/// stops at the block footer.
/// 渲染单条暂存输出清单条目。完整显示的结果只需起始行；尚未完全看到的结果
/// 给出 `offset`/`limit`，范围从首个未显示行往前两行起、到该结果块的末尾止，
/// 使续读重复一点上下文并停在块尾部。
fn persisted_entry(label: &str, range: &PartRange, first_hidden: Option<usize>) -> String {
    let line = range.start.to_string();
    let Some(hidden) = first_hidden else {
        return t_fmt(
            "truncated_persisted_entry",
            &[("tool", label), ("line", &line)],
        );
    };

    let offset = hidden.saturating_sub(CONTEXT_LINES_BEFORE_CUT).max(1);
    let limit = range.end.saturating_sub(offset) + 1;
    t_fmt(
        "truncated_persisted_entry_resume",
        &[
            ("tool", label),
            ("line", &line),
            ("offset", &offset.to_string()),
            ("limit", &limit.to_string()),
        ],
    )
}

/// The first line of `part`'s block that a truncated output does not show.
/// The header occupies its own lines; the kept prefix ends on the line where
/// the cut fell, so that whole line is unseen from the cut point onwards.
/// 截断输出未显示的、`part` 块的首个行号。头部独占若干行；保留前缀结束于
/// 截断点所在行，因此该整行自截断点起即未显示。
fn first_hidden_line(part: &ResultPart, range: &PartRange, kept_chars: usize) -> usize {
    let kept_newlines = part
        .content
        .chars()
        .take(kept_chars)
        .filter(|c| *c == '\n')
        .count();
    range.start + part.header.matches('\n').count() + kept_newlines
}

/// Lowercase hex encoding of `SHA-256(content)` (64 hex characters).
/// `SHA-256(content)` 的小写十六进制编码（64 个十六进制字符）。
fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// The span of lines one result block occupies in the persisted temp file.
/// 单个结果块在暂存临时文件中占用的行范围。
struct PartRange {
    /// First line of the block, its `[TOOL_RESULT ...]` header included.
    /// 块的首行，含 `[TOOL_RESULT ...]` 头部。
    start: usize,
    /// Last line holding any of the block's characters, footer included.
    /// 含块中字符的末行，含尾部。
    end: usize,
}

/// Persist the complete un-truncated tool outputs to
/// `<workspace_root>/.ManualAid/temp/<sha256>.md`. Returns the written file
/// path and the 1-based line span of each part within that file, or `None`
/// when the directory cannot be created or the write fails.
/// 将未截断的完整工具输出写入 `<workspace_root>/.ManualAid/temp/<sha256>.md`。
/// 返回写入的文件路径以及每个部分在该文件中的 1 基行范围；目录创建或
/// 写入失败时返回 `None`。
fn persist_full_output(
    parts: &[ResultPart],
    workspace_root: &Path,
) -> Option<(PathBuf, Vec<PartRange>)> {
    let separator = "\n\n";
    let mut full = String::new();
    let mut ranges = Vec::with_capacity(parts.len());
    // Advance a running line number instead of recounting `full` per part:
    // the separator must be accounted for *before* a block's first line is
    // recorded, and counting the accumulated text would make it quadratic.
    // 用运行中的行号推进，而不是逐块重数 `full`：分隔符必须在记录块的
    // 首行之前计入，且重复统计累积文本会退化为平方复杂度。
    let mut line = 1usize;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            full.push_str(separator);
            line += separator.matches('\n').count();
        }
        let block = format!("{}{}{}", part.header, part.content, part.footer);
        let newlines = block.matches('\n').count();
        let start = line;
        // Blocks close with the footer's `]`, so a block never ends on a
        // newline; the guard keeps the span correct if that ever changes.
        // 块以尾部的 `]` 收束，故永不结束于换行；该保护使行范围在假设
        // 变化时仍然正确。
        let end = start + newlines - usize::from(block.ends_with('\n'));
        ranges.push(PartRange { start, end });
        full.push_str(&block);
        // The next character lands one line further on per newline emitted.
        // 每输出一个换行，下一个字符所在行号前进一行。
        line = start + newlines;
    }

    let temp_dir = workspace_root.join(".ManualAid").join("temp");
    std::fs::create_dir_all(&temp_dir).ok()?;
    let file_path = temp_dir.join(format!("{}.md", sha256_hex(&full)));
    std::fs::write(&file_path, &full).ok()?;
    Some((file_path, ranges))
}

/// Render the opening bracket line of a tool result. The parameter summary
/// is already a single-line truncated JSON string, so no escaping is needed.
/// User-driven actions render a `[USER_ACTION]` header instead; see
/// [`UserAction`].
/// 渲染工具结果的开括号行。参数摘要已是单行截断 JSON 字符串，无需转义。
/// 用户驱动操作改为渲染 `[USER_ACTION]` 头部；见 [`UserAction`]。
fn result_header(result: &ToolResult) -> String {
    if let Some(action) = &result.user_action {
        let label = escape_user_action_label(&action.label);
        return format!(
            "[USER_ACTION kind=\"{}\" {}=\"{}\"]\n",
            action.kind.as_str(),
            action.kind.attr_name(),
            label,
        );
    }

    let params_attr = if result.params_summary.is_empty() {
        String::new()
    } else {
        format!(" params={}", result.params_summary)
    };
    format!(
        "[TOOL_RESULT {tool} success={success}{params_attr}]\n",
        tool = result.tool_name,
        success = result.success,
        params_attr = params_attr,
    )
}

/// Render the closing bracket line of a tool result. User-driven actions use
/// a fixed `[END USER_ACTION]` footer without a trailing name.
/// 渲染工具结果的闭括号行。用户驱动操作使用固定 `[END USER_ACTION]` 尾部，
/// 不带尾随名称。
fn result_footer(result: &ToolResult) -> String {
    if result.user_action.is_some() {
        return "\n[END USER_ACTION]".to_string();
    }
    format!("\n[END TOOL_RESULT {}]", result.tool_name)
}

/// Escape a user-action label so the `[USER_ACTION]` header always stays on
/// one line. Backslash is escaped first so a trailing directory marker `\`
/// renders as `\\`; when the attribute value is later unescaped it returns to
/// a single backslash without becoming ambiguous with `\"`.
/// 转义用户操作 label，保证 `[USER_ACTION]` 头部恒为单行。先转义反斜杠，
/// 因此目录尾标记 `\` 渲染为 `\\`；属性值反转义后还原为单个反斜杠，不与
/// `\"` 产生歧义。
fn escape_user_action_label(label: &str) -> String {
    let mut escaped = label.replace('\\', "\\\\");
    escaped = escaped.replace('"', "\\\"");
    escaped = escaped.replace('\n', "\\n");
    escaped = escaped.replace('\r', "\\r");
    escaped = escaped.replace('\t', "\\t");
    escaped
}

/// Translate `key` and replace `%{name}` placeholders.
/// 翻译 `key` 并替换 `%{name}` 占位符。
fn t_fmt(key: &str, args: &[(&str, &str)]) -> String {
    let mut template = i18n::t_str(key);
    for (name, value) in args {
        template = template.replace(&format!("%{{{name}}}"), value);
    }
    template
}

#[cfg(test)]
mod tests {
    use super::*;
    use manualaid_core::tools::UserActionKind;

    #[test]
    fn renders_tools_list_with_templates() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let list = render_tools_list(&config, &registry);
        assert!(list.contains("## read"));
        assert!(list.contains("Call template"));
        assert!(list.contains("\"tool_use\": \"read\""));
        assert!(!list.contains("## shell") || config.shell);
    }

    #[test]
    fn disabled_tools_are_omitted() {
        let config = Config {
            shell: false,
            ..Config::default()
        };
        let registry = FormatRegistry::new();
        let list = render_tools_list(&config, &registry);
        assert!(!list.contains("## shell"));
    }

    #[test]
    fn tools_list_reminder_wraps_full_list() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let wrapped = render_tools_list_reminder(&config, &registry);
        assert!(wrapped.starts_with("<system-reminder>\n"));
        assert!(wrapped.ends_with("</system-reminder>"));
        assert!(wrapped.contains("## read"));
        assert!(wrapped.contains("Call template"));
    }

    #[test]
    fn skills_list_reminder_wraps_enabled_skills_and_placeholder() {
        let disabled = vec![Skill {
            unique_name: "greeter".into(),
            name: "greeter".into(),
            description: "says hi".into(),
            body: "## Usage\nhi".into(),
            path: std::path::PathBuf::from("/skills/greeter"),
            agent_dir: ".claude".into(),
            is_global: true,
            is_enabled: false,
        }];
        let empty = render_skills_list(&disabled);
        assert!(empty.starts_with("<system-reminder>\n"));
        assert!(empty.contains(&i18n::t_str("prompt.copy.skills-empty")));

        let enabled = vec![Skill {
            is_enabled: true,
            ..disabled[0].clone()
        }];
        let list = render_skills_list(&enabled);
        assert!(list.starts_with("<system-reminder>\n"));
        assert!(list.contains("<available_skills>"));
        assert!(list.ends_with("</system-reminder>"));
    }

    #[test]
    fn system_prompt_contains_context_and_skills() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[], &[]);
        assert!(prompt.starts_with("<system_prompt>"));
        assert!(prompt.ends_with("</system_prompt>"));
        assert!(prompt.contains("<workspace_root>"));
        assert!(prompt.contains("<shell_environment>"));
    }

    #[test]
    fn system_prompt_drops_the_legacy_task_planning_rule() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[], &[]);
        assert!(prompt.contains(&i18n::t_str("prompt.system.system-reminder-note")));
        // TODO planning moved into the `todo_write` tool, so the prompt-only
        // rule must no longer be injected.
        // TODO 规划已迁入 `todo_write` 工具，仅靠提示词的规则不应再注入。
        assert!(!prompt.contains(&i18n::t_str("prompt.copy.task-planning-rule")));
    }

    /// Self-cleaning temporary workspace root.
    /// 自清理的临时工作区根。
    struct TempRoot(std::path::PathBuf);

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("manualaid-ws-prompt-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("create temp root");
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn todo_context_is_injected_only_while_the_tool_is_enabled() {
        let root = TempRoot::new("todo-context");
        let plans = root.0.join(".ManualAid").join("plans");
        let todos = root.0.join(".ManualAid").join("todos");
        std::fs::create_dir_all(&plans).expect("create plans dir");
        std::fs::create_dir_all(&todos).expect("create todos dir");
        std::fs::write(plans.join("plan-a.md"), "# plan\n").expect("write plan");
        std::fs::write(
            todos.join("alpha.json"),
            r#"{"subject":"alpha","create_datetime":"2026-01-01T00:00:00+00:00","update_datetime":"2026-01-01T00:00:00+00:00","linked_plan":"plan-a","todos":[{"task":"a","status":"pending"}]}"#,
        )
        .expect("write todo");

        let registry = FormatRegistry::new();
        let enabled = build_system_prompt(&Config::default(), &root.0, &registry, &[], &[]);
        assert!(enabled.contains("<unfinished_todos>"));
        assert!(enabled.contains("\nalpha: 0%\n"));
        assert!(enabled.contains("</unfinished_todos>"));
        // The injected block never carries a `<system-reminder>` wrapper; only
        // the copied context does.
        // 注入的区块不带 `<system-reminder>` 包裹；只有复制的上下文才带。
        assert!(!enabled.contains("<system-reminder>\n<unfinished_todos>"));
        assert!(!enabled.contains("%{todo_context}"));

        let off = Config {
            todo_write: false,
            ..Config::default()
        };
        let disabled = build_system_prompt(&off, &root.0, &registry, &[], &[]);
        assert!(!disabled.contains("<unfinished_todos>"));
        assert!(!disabled.contains("%{todo_context}"));
    }

    #[test]
    fn system_prompt_contains_user_action_note() {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("/ws"), &registry, &[], &[]);
        let note = i18n::t_str("prompt.system.user-action-note");
        assert!(!note.is_empty());
        assert!(prompt.contains(&note));
    }

    #[test]
    fn tool_calling_format_description_contains_read_template_and_notes() {
        let registry = FormatRegistry::new();
        let description = tool_calling_format_description(&registry);
        assert!(description.contains(&i18n::t_str("cli.prompt.func_calls_notes")));
        assert!(description.contains("\"tool_use\": \"read\""));
    }

    #[test]
    fn platform_notes_text_for_covers_both_platform_decisions() {
        let windows_notes = platform_notes_text_for(true, false);
        assert!(windows_notes.starts_with("<platform-notes>"));
        assert!(windows_notes.contains("Windows"));
        assert!(!windows_notes.contains("CoreUtils"));
        assert!(platform_notes_text_for(false, true).is_empty());
        assert!(platform_notes_text_for(false, false).is_empty());
    }

    #[test]
    fn platform_notes_text_for_appends_coreutils_note_when_installed() {
        let with_note = platform_notes_text_for(true, true);
        assert!(with_note.starts_with("<platform-notes>"));
        assert!(with_note.contains("</platform-notes>"));
        assert!(with_note.contains("CoreUtils"));
        assert!(with_note.contains("coreutils.exe"));
        // The injected note must sit inside the block, between the Windows
        // bullets and the closing tag.
        // 注入的备注必须位于块内：在 Windows 条目与闭合标签之间。
        let windows_idx = with_note.find("Windows").unwrap();
        let coreutils_idx = with_note.find("CoreUtils").unwrap();
        let closing_idx = with_note.find("</platform-notes>").unwrap();
        assert!(windows_idx < coreutils_idx);
        assert!(coreutils_idx < closing_idx);
    }

    #[test]
    fn platform_notes_text_for_omits_coreutils_note_when_not_installed() {
        let without_note = platform_notes_text_for(true, false);
        assert!(!without_note.contains("CoreUtils"));
        assert!(!without_note.contains("coreutils.exe"));
    }

    #[cfg(windows)]
    #[test]
    fn platform_notes_are_non_empty_on_windows() {
        let notes = platform_notes_text();
        assert!(notes.starts_with("<platform-notes>"));
        assert!(notes.contains("Windows"));
    }

    #[cfg(not(windows))]
    #[test]
    fn platform_notes_are_empty_elsewhere() {
        assert!(platform_notes_text().is_empty());
    }

    #[test]
    fn push_section_appends_only_non_empty_sections() {
        let mut out = String::from("head");
        push_section(&mut out, "body");
        assert_eq!(out, "head\nbody");
        let mut out = String::from("head");
        push_section(&mut out, "");
        assert_eq!(out, "head");
    }

    #[test]
    fn result_header_includes_summary_when_present() {
        let result = ToolResult::success("read", "content", true)
            .with_params_summary("{\"file_path\":\"/a.txt\"}".into());
        assert_eq!(
            result_header(&result),
            "[TOOL_RESULT read success=true params={\"file_path\":\"/a.txt\"}]\n"
        );
    }

    #[test]
    fn result_header_omits_summary_when_empty() {
        let result = ToolResult::success("shell", "done", false);
        assert_eq!(result_header(&result), "[TOOL_RESULT shell success=true]\n");
    }

    #[test]
    fn result_footer_closes_with_tool_name() {
        let result = ToolResult::failure("edit", "boom");
        assert_eq!(result_footer(&result), "\n[END TOOL_RESULT edit]");
    }

    #[test]
    fn skills_list_text_is_empty_when_nothing_is_enabled() {
        let skills = vec![Skill {
            unique_name: "greeter".into(),
            name: "greeter".into(),
            description: "says hi".into(),
            body: "## Usage\nhi".into(),
            path: std::path::PathBuf::from("/skills/greeter"),
            agent_dir: ".claude".into(),
            is_global: true,
            is_enabled: false,
        }];
        assert_eq!(skills_list_text(&skills), "");
    }

    #[test]
    fn result_header_renders_user_action_exec() {
        let result = ToolResult::success("shell", "out", false)
            .with_user_action(UserActionKind::Exec, "cargo test");
        assert_eq!(
            result_header(&result),
            "[USER_ACTION kind=\"exec\" command=\"cargo test\"]\n"
        );
    }

    #[test]
    fn result_header_renders_user_action_skill_and_path() {
        let skill = ToolResult::success("skill", "out", true)
            .with_user_action(UserActionKind::Skill, "plan");
        assert_eq!(
            result_header(&skill),
            "[USER_ACTION kind=\"skill\" name=\"plan\"]\n"
        );

        let path = ToolResult::success("read", "out", true)
            .with_user_action(UserActionKind::Path, "E:/ProjectRust/ManualAid-Rust/.git\\");
        assert_eq!(
            result_header(&path),
            "[USER_ACTION kind=\"path\" path=\"E:/ProjectRust/ManualAid-Rust/.git\\\\\"]\n"
        );
    }

    #[test]
    fn user_action_footer_is_fixed_without_trailing_name() {
        let result = ToolResult::success("shell", "out", false)
            .with_user_action(UserActionKind::Exec, "cargo test");
        assert_eq!(result_footer(&result), "\n[END USER_ACTION]");
    }

    #[test]
    fn user_action_label_escapes_quote_backslash_and_newlines() {
        let result = ToolResult::success("shell", "out", false)
            .with_user_action(UserActionKind::Exec, "echo \"hi\"\\nline");
        assert_eq!(
            result_header(&result),
            "[USER_ACTION kind=\"exec\" command=\"echo \\\"hi\\\"\\\\nline\"]\n"
        );
    }

    #[test]
    fn regular_result_header_is_unchanged_with_user_action_none() {
        let result = ToolResult::success("read", "content", true);
        assert_eq!(result_header(&result), "[TOOL_RESULT read success=true]\n");
        assert_eq!(result_footer(&result), "\n[END TOOL_RESULT read]");
    }
}
