//! The interactive Agent Copy-Paste Loop: generate a workspace system
//! prompt, parse and execute tool-call text pasted from (or typed into)
//! the console with per-item audit approvals, and copy the results back to
//! the clipboard for an external LLM chat.
//! 交互式 Agent Copy-Paste Loop：生成工作区系统提示词，解析并执行从
//! 剪贴板粘贴或手动输入的工具调用文本（带逐项审计批准），并把结果复制
//! 回剪贴板供外部 LLM 聊天使用。

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use indexmap::IndexMap;
use manualaid_core::audit::{AuditDecision, AuditQueueItem, Auditor};
use manualaid_core::executor::Executor;
use manualaid_core::parser::{FormatRegistry, ParsedToolCall, RegistryMode};
use manualaid_core::skill::{all_skills, reload_skills, set_enabled};
use manualaid_core::tools::{ToolKind, ToolResult, params_summary_of};
use manualaid_core::user_dir::home_dir;
use manualaid_ws::config::{Config, save_project};
use manualaid_ws::session::SessionLog;
use serde_json::Value;

/// How the user answered one approval-queue item.
/// 用户对单个审批队列项的答复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Approval {
    /// Approve the operation.
    /// 同意该操作。
    Approve,
    /// Deny the operation.
    /// 拒绝该操作。
    Deny,
    /// Deny the operation and return the typed text as the tool result.
    /// 拒绝该操作，并把键入的文本作为工具调用结果返回。
    DenyWithText(String),
}

/// Session-level loop switches (not persisted).
/// 会话级 loop 开关（不持久化）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopOptions {
    /// Whether results are copied to the clipboard automatically after an
    /// executed round.
    /// 每轮执行后是否自动把结果复制到剪贴板。
    pub auto_copy: bool,
    /// Whether the screen is cleared before each menu render.
    /// 每次渲染菜单前是否清屏。
    pub clear_screen: bool,
}

impl Default for LoopOptions {
    fn default() -> Self {
        Self {
            auto_copy: true,
            clear_screen: false,
        }
    }
}

/// Run the interactive loop with a new Tokio runtime; the caller remains
/// synchronous.
/// 用新建的 Tokio runtime 运行交互式 loop；调用方保持同步。
pub fn run_loop(home: Option<&Path>, lang: Option<String>) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to build runtime: {e}"))?;
    runtime.block_on(loop_main(home, lang))
}

/// The interactive loop body (async because tool execution is async).
/// 交互式 loop 主体（异步，因为工具执行是异步的）。
async fn loop_main(home: Option<&Path>, lang: Option<String>) -> Result<(), String> {
    let current_dir = std::env::current_dir()
        .map_err(|e| t_fmt("cli.error.current_dir", &[("error", &e.to_string())]))?;
    let home = match home {
        Some(home) => home.to_path_buf(),
        None => home_dir().map_err(|e| e.to_string())?,
    };

    let mut config = manualaid_ws::config::load(&current_dir, &home)
        .map_err(|e| t_fmt("cli.error.init", &[("error", &e.to_string())]))?;
    apply_cli_lang(lang, &mut config);
    i18n::set_locale(&config.lang);

    reload_skills(&current_dir).map_err(|e| e.to_string())?;

    let registry = FormatRegistry::new();
    apply_format_mode(&registry, &config)?;

    let auditor =
        Auditor::new(current_dir.clone()).with_allowed_commands(config.allow_commands.clone());
    let executor = Executor::new(auditor, Arc::new(None));
    let mut session = SessionLog::new();
    let mut options = LoopOptions::default();

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let shell = manualaid_core::shell::detected_shell();
    println!(
        "{}",
        t_fmt(
            "cli.loop.header",
            &[
                ("path", &current_dir.display().to_string()),
                ("time", &now),
                ("shell", &shell),
            ],
        )
    );

    let mut should_exit = false;
    while !should_exit {
        if options.clear_screen {
            clear_screen();
        }
        let _ = crate::pager::print_paged(&render_menu());
        print!("{}", i18n::t_str("cli.loop.menu_prompt"));
        let _ = std::io::stdout().flush();

        let line = match read_line() {
            Some(line) => line,
            None => break,
        };
        let trimmed = line.trim();
        if trimmed.starts_with('/') {
            handle_inline_command(&mut config, &registry, &current_dir, &mut session, trimmed);
            continue;
        }
        match trimmed {
            "1" => copy_system_prompt(&config, &current_dir, &registry),
            "2" => paste_and_submit(&executor, &registry, &mut session, &mut options).await,
            "3" => input_and_submit(&executor, &registry, &mut session, &mut options).await,
            "4" => copy_round_result(&session),
            "5" => config_menu(&mut config, &registry, &current_dir, &mut options),
            "6" => print_session_summary(&config, &session),
            "0" => should_exit = true,
            _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
        }
        if !should_exit && options.clear_screen {
            // Keep the previous action's output readable before the next
            // screen clear.
            // 在下次清屏前保留上一步输出的可读时间。
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    Ok(())
}

/// Apply an explicit `-l/--lang` override to the session config. Invalid
/// values are ignored so a typo never breaks the loop startup.
/// 把显式传入的 `-l/--lang` 覆盖到会话配置。非法值会被忽略，避免拼写
/// 错误导致 loop 无法启动。
fn apply_cli_lang(cli_lang: Option<String>, config: &mut Config) {
    if let Some(lang) = cli_lang.filter(|lang| Config::is_valid_lang(lang)) {
        config.lang = lang;
    }
}

/// Render the main menu text.
/// 渲染主菜单文本。
pub fn render_menu() -> String {
    [
        "cli.loop.menu_title",
        "cli.loop.menu_generate",
        "cli.loop.menu_paste",
        "cli.loop.menu_input",
        "cli.loop.menu_copy",
        "cli.loop.menu_config",
        "cli.loop.menu_summary",
        "cli.loop.menu_exit",
    ]
    .iter()
    .map(|key| i18n::t_str(key))
    .collect::<Vec<_>>()
    .join("\n")
        + "\n"
}

/// Parse and execute one round of tool calls with user approval.
///
/// Every call is pre-checked first: calls guaranteed to fail produce a
/// failure result directly. Remaining calls are audited and the ones
/// needing approval are presented one by one; `decide` returns the user's
/// answer for each item. Approved calls execute, denied calls become
/// failure results. Returns the parsed calls and results of the round.
/// 解析并执行一轮带用户审批的工具调用。
///
/// 每个调用都会先经过预检：必然失败的调用直接产生失败结果。其余调用
/// 进入审计，需要批准的项目逐条展示；`decide` 返回用户对每一项的答复。
/// 已批准的调用正常执行，被拒绝的调用生成失败结果。返回本轮解析出的
/// 调用与结果。
pub async fn execute_round_with_approval(
    executor: &Executor,
    registry: &FormatRegistry,
    input: &str,
    mut decide: impl FnMut(&AuditQueueItem) -> Approval,
) -> Result<(Vec<ParsedToolCall>, Vec<ToolResult>), String> {
    let calls = registry
        .parse(input)
        .map_err(|e| t_fmt("cli.error.parse", &[("error", &e.to_string())]))?;
    if calls.is_empty() {
        return Err(i18n::t_str("cli.error.no_calls"));
    }
    let parsed_calls = calls.clone();

    struct AuditedCall {
        call: ParsedToolCall,
        pre_failed: Option<ToolResult>,
        pending: Vec<(String, AuditDecision)>,
    }

    let mut audited = Vec::with_capacity(calls.len());
    let mut queue_len = 0usize;
    for call in calls {
        let pre_failed = executor.pre_check(&call).await;
        let pending = if pre_failed.is_some() {
            Vec::new()
        } else {
            executor
                .audit(&call)
                .into_iter()
                .filter(|(_, decision)| matches!(decision, AuditDecision::NeedsApproval(_)))
                .collect()
        };
        queue_len += pending.len();
        audited.push(AuditedCall {
            call,
            pre_failed,
            pending,
        });
    }

    let mut approved = vec![true; audited.len()];
    let mut denied_texts = vec![None; audited.len()];
    if queue_len > 0 {
        println!(
            "{}",
            t_fmt("cli.audit.header", &[("count", &queue_len.to_string())])
        );
        for (index, item) in audited.iter_mut().enumerate() {
            if item.pre_failed.is_some() {
                continue;
            }
            for (param, decision) in &item.pending {
                let queue_item = AuditQueueItem {
                    tool_name: item.call.tool_name.clone(),
                    param_name: param.clone(),
                    decision: decision.clone(),
                };
                println!("{}", approval_preview(&queue_item, &item.call.params));
                match decide(&queue_item) {
                    Approval::Approve => {}
                    Approval::Deny => approved[index] = false,
                    Approval::DenyWithText(text) => {
                        approved[index] = false;
                        denied_texts[index] = Some(text);
                    }
                }
            }
        }
    }

    let mut results = Vec::with_capacity(audited.len());
    for (index, item) in audited.into_iter().enumerate() {
        if let Some(pre_failed) = item.pre_failed {
            results.push(pre_failed);
            continue;
        }
        if !approved[index] {
            let decision = item.pending.first().map(|(_, d)| d);
            results.push(denied_result(
                &item.call,
                decision,
                denied_texts[index].clone(),
            ));
            continue;
        }
        let mut result = executor.execute(item.call).await;
        if !item.pending.is_empty() {
            // An approved call no longer needs the "approval needed"
            // annotation in its summary.
            // 已批准的调用不再在摘要中标注"需要批准"。
            result.audit_decisions.clear();
        }
        results.push(result);
    }
    Ok((parsed_calls, results))
}

/// Build the failure result of a denied call.
/// 构建被拒绝调用的失败结果。
fn denied_result(
    call: &ParsedToolCall,
    decision: Option<&AuditDecision>,
    denied_text: Option<String>,
) -> ToolResult {
    let output = if let Some(text) = denied_text {
        text
    } else {
        match decision {
            Some(AuditDecision::NeedsApproval(reason)) | Some(AuditDecision::Denied(reason)) => {
                t_fmt("cli.approval.denied_result", &[("reason", reason)])
            }
            _ => i18n::t_str("cli.approval.denied_result"),
        }
    };
    ToolResult::failure(&call.tool_name, output)
        .with_params_summary(params_summary_of(&call.params))
}

/// Render the console summary of one round's results.
/// 渲染一轮执行结果的控制台摘要。
pub fn format_round_summary(results: &[ToolResult]) -> String {
    let mut out = String::new();
    for result in results {
        let state = if result.success {
            i18n::t_str("cli.message.success")
        } else {
            i18n::t_str("cli.message.failure")
        };
        out.push_str(&format!(
            "[{}] {state}\n{}\n",
            result.tool_name,
            result.output.trim()
        ));
        for (param, decision) in &result.audit_decisions {
            if let Some(reason) = decision.reason() {
                out.push_str(&format!(
                    "  {}: {param} ({reason})\n",
                    i18n::t_str("cli.message.approval_needed")
                ));
            }
        }
    }
    out
}

/// Parse a round index input (`1` = latest). Empty input means `1`; an
/// out-of-range or non-numeric value yields `None`.
/// 解析批次索引输入（`1` = 最新）。空输入表示 `1`；越界或非数字返回
/// `None`。
pub fn parse_round_index(input: &str, total: usize) -> Option<usize> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(1);
    }
    let index: usize = trimmed.parse().ok()?;
    (1..=total).contains(&index).then_some(index)
}

/// Cycle the interface language between `en` and `zh-CN`.
/// 在 `en` 与 `zh-CN` 之间循环切换界面语言。
pub fn cycle_lang(current: &str) -> String {
    if current == "en" {
        "zh-CN".to_string()
    } else {
        "en".to_string()
    }
}

/// Cycle the tool-call format through `auto` → `xml` → `json-codeblock`.
/// 按 `auto` → `xml` → `json-codeblock` 循环切换工具调用格式。
pub fn cycle_format(current: &str) -> String {
    let labels = RegistryMode::all_labels();
    let index = labels
        .iter()
        .position(|label| *label == current)
        .unwrap_or(0);
    labels[(index + 1) % labels.len()].to_string()
}

/// Apply the configured format label to the registry.
/// 将配置的格式标签应用到注册表。
fn apply_format_mode(registry: &FormatRegistry, config: &Config) -> Result<(), String> {
    let mode = RegistryMode::from_label(&config.tool_call_format)
        .ok_or_else(|| format!("Unknown format label `{}`", config.tool_call_format))?;
    registry.set_mode(mode).map_err(|e| e.to_string())
}

/// Ask the user whether to approve one operation (`y` / `n` / `t`).
/// 询问用户是否批准某一操作（`y` / `n` / `t`）。
fn ask_approval(item: &AuditQueueItem) -> Approval {
    print!(
        "{}",
        t_fmt(
            "cli.audit.prompt",
            &[("tool", &item.tool_name), ("param", &item.param_name),],
        )
    );
    let _ = std::io::stdout().flush();
    match read_line().map(|line| line.trim().to_ascii_lowercase()) {
        Some(line) if line == "y" => Approval::Approve,
        Some(line) if line == "t" => {
            let text = read_line().unwrap_or_default();
            Approval::DenyWithText(text.trim().to_string())
        }
        _ => Approval::Deny,
    }
}

/// Generate the system prompt and copy it to the clipboard.
/// 生成系统提示词并复制到剪贴板。
fn copy_system_prompt(config: &Config, root: &Path, registry: &FormatRegistry) {
    let start = std::time::Instant::now();
    let text = manualaid_ws::prompt::build_system_prompt(config, root, registry, &all_skills());
    match manualaid_core::clipboard::write_clipboard(&text) {
        Ok(()) => println!("{}", i18n::t_str("cli.message.prompt_copied")),
        Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
    }
    println!(
        "{}",
        t_fmt(
            "cli.loop.timing_prompt",
            &[("elapsed", &crate::format_duration(start.elapsed()))]
        )
    );
}

/// Read the clipboard and submit its text as one round.
/// 读取剪贴板并把其文本作为一轮提交。
async fn paste_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
) {
    let text = match manualaid_core::clipboard::read_clipboard() {
        Ok(text) if text.trim().is_empty() => {
            println!("{}", i18n::t_str("cli.message.clipboard_empty"));
            return;
        }
        Ok(text) => text,
        Err(e) => {
            eprintln!("{}", t_fmt("cli.error.clipboard_read", &[("error", &e)]));
            return;
        }
    };
    submit_text(executor, registry, session, options, &text).await;
}

/// Read multi-line text from stdin (until EOF) and submit it as one round.
/// 从标准输入读取多行文本（直到 EOF）并作为一轮提交。
async fn input_and_submit(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
) {
    println!("{}", i18n::t_str("cli.message.input_prompt"));
    let mut text = String::new();
    for line in std::io::stdin().lock().lines() {
        match line {
            Ok(line) => {
                text.push_str(&line);
                text.push('\n');
            }
            Err(_) => break,
        }
    }
    if text.trim().is_empty() {
        return;
    }
    submit_text(executor, registry, session, options, &text).await;
}

/// Parse and execute input, print the summary and copy results when asked.
/// 解析并执行输入，打印摘要并按需复制结果。
async fn submit_text(
    executor: &Executor,
    registry: &FormatRegistry,
    session: &mut SessionLog,
    options: &mut LoopOptions,
    text: &str,
) {
    let round_start = std::time::Instant::now();
    match execute_round_with_approval(executor, registry, text, ask_approval).await {
        Ok((calls, results)) => {
            let _ = crate::pager::print_paged(&format_round_summary(&results));
            session.push(calls, results.clone());
            let round_index = session.len();
            let copy = options.auto_copy || ask_copy();
            if copy
                && let Err(e) = manualaid_core::clipboard::write_clipboard(
                    manualaid_ws::prompt::format_results(&results),
                )
            {
                eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]));
            } else if copy {
                println!(
                    "{}",
                    t_fmt(
                        "cli.message.result_copied",
                        &[("index", &round_index.to_string())]
                    )
                );
            }
            println!(
                "{}",
                t_fmt(
                    "cli.loop.timing_round",
                    &[("elapsed", &crate::format_duration(round_start.elapsed()))]
                )
            );
        }
        Err(e) => eprintln!("{e}"),
    }
}

/// Ask whether to copy the round results to the clipboard.
/// 询问是否将本轮结果复制到剪贴板。
fn ask_copy() -> bool {
    print!("{}", i18n::t_str("cli.message.ask_copy"));
    let _ = std::io::stdout().flush();
    read_line().is_some_and(|line| line.trim().eq_ignore_ascii_case("y"))
}

/// Copy the `index`-th latest round (default: latest) to the clipboard.
/// 把从最新算起的第 `index` 轮（默认最新）复制到剪贴板。
fn copy_round_result(session: &SessionLog) {
    if session.is_empty() {
        println!("{}", i18n::t_str("cli.message.no_rounds"));
        return;
    }
    println!(
        "{}",
        t_fmt(
            "cli.message.round_count",
            &[("count", &session.len().to_string())]
        )
    );
    print!("{}", i18n::t_str("cli.message.copy_index_prompt"));
    let _ = std::io::stdout().flush();
    let input = read_line().unwrap_or_default();
    match parse_round_index(&input, session.len()) {
        Some(index) => {
            let results = &session.latest(index).expect("validated index").results;
            match manualaid_core::clipboard::write_clipboard(manualaid_ws::prompt::format_results(
                results,
            )) {
                Ok(()) => println!(
                    "{}",
                    t_fmt(
                        "cli.message.result_copied",
                        &[("index", &index.to_string())]
                    )
                ),
                Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
            }
        }
        None => println!(
            "{}",
            t_fmt(
                "cli.error.invalid_index",
                &[("count", &session.len().to_string())]
            )
        ),
    }
}

/// Print the session summary (round count, tool-call count, enabled tools).
/// 打印会话摘要（批次数量、工具调用数量、已启用工具）。
fn print_session_summary(config: &Config, session: &SessionLog) {
    let tools = config.enabled_tool_names().join(", ");
    let text = [
        crate::style::header(&i18n::t_str("cli.message.summary_title")),
        t_fmt(
            "cli.message.summary_rounds",
            &[("count", &session.len().to_string())],
        ),
        t_fmt(
            "cli.message.summary_tool_calls",
            &[("count", &session.total_calls().to_string())],
        ),
        t_fmt("cli.message.summary_enabled_tools", &[("tools", &tools)]),
    ]
    .join("\n");
    let _ = crate::pager::print_paged(&text);
}

/// The secondary configuration menu.
/// 二级配置菜单。
fn config_menu(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    options: &mut LoopOptions,
) {
    loop {
        println!("{}", render_config_menu(config, options));
        let line = read_line().unwrap_or_default();
        match line.trim() {
            "1" => {
                config.lang = cycle_lang(&config.lang);
                i18n::set_locale(&config.lang);
                persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
            }
            "2" => {
                config.tool_call_format = cycle_format(&config.tool_call_format);
                let _ = apply_format_mode(registry, config);
                persist_and_confirm(
                    config,
                    root,
                    "cli.config.format_switched",
                    &config.tool_call_format,
                );
            }
            "3" => toggle_tool(config, root, "shell"),
            "4" => toggle_tool(config, root, "read"),
            "5" => toggle_tool(config, root, "write"),
            "6" => toggle_tool(config, root, "edit"),
            "7" => toggle_tool(config, root, "skill"),
            "8" => options.auto_copy = !options.auto_copy,
            "9" => options.clear_screen = !options.clear_screen,
            "10" => skill_config_menu(),
            "0" | "" => break,
            _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
        }
    }
}

/// Toggle one tool switch and persist the configuration.
/// 切换一个工具开关并持久化配置。
fn toggle_tool(config: &mut Config, root: &Path, tool: &str) {
    match tool {
        "shell" => config.shell = !config.shell,
        "read" => config.read = !config.read,
        "write" => config.write = !config.write,
        "edit" => config.edit = !config.edit,
        "skill" => config.skill = !config.skill,
        _ => return,
    }
    persist_and_confirm(config, root, "cli.config.saved", "");
}

/// Persist the config and print a confirmation message.
/// 持久化配置并打印确认消息。
fn persist_and_confirm(config: &Config, root: &Path, key: &str, value: &str) {
    match save_project(root, config) {
        Ok(()) => println!(
            "{}",
            t_fmt(key, &[("lang", value), ("format", value), ("value", value)])
        ),
        Err(e) => eprintln!(
            "{}",
            t_fmt("cli.error.output", &[("error", &e.to_string())])
        ),
    }
}

/// Render the configuration menu with current states.
/// 渲染带当前状态的配置菜单。
pub fn render_config_menu(config: &Config, options: &LoopOptions) -> String {
    let lang_name = if config.lang == "en" {
        "English"
    } else {
        "中文"
    };
    let state = |enabled: bool| {
        if enabled {
            i18n::t_str("cli.config.enabled")
        } else {
            i18n::t_str("cli.config.disabled")
        }
    };
    [
        i18n::t_str("cli.config.title"),
        t_fmt("cli.config.lang", &[("lang", lang_name)]),
        t_fmt("cli.config.format", &[("format", &config.tool_call_format)]),
        t_fmt("cli.config.shell", &[("state", &state(config.shell))]),
        t_fmt("cli.config.read", &[("state", &state(config.read))]),
        t_fmt("cli.config.write", &[("state", &state(config.write))]),
        t_fmt("cli.config.edit", &[("state", &state(config.edit))]),
        t_fmt("cli.config.skill", &[("state", &state(config.skill))]),
        t_fmt(
            "cli.config.auto_copy",
            &[("state", &state(options.auto_copy))],
        ),
        t_fmt(
            "cli.config.clear_screen",
            &[("state", &state(options.clear_screen))],
        ),
        i18n::t_str("cli.config.skill_list"),
        i18n::t_str("cli.config.back"),
    ]
    .join("\n")
}

/// The SKILL enable/disable sub-menu: toggle by index, all on, all off.
/// SKILL 启用/禁用二级菜单：按索引切换、全部启用、全部禁用。
fn skill_config_menu() {
    loop {
        let skills = all_skills();
        let mut lines = vec![i18n::t_str("cli.skill_config.title")];
        for (index, skill) in skills.iter().enumerate() {
            let state = if skill.is_enabled {
                i18n::t_str("cli.config.enabled")
            } else {
                i18n::t_str("cli.config.disabled")
            };
            lines.push(t_fmt(
                "cli.skill_config.item",
                &[
                    ("state", &state),
                    ("index", &(index + 1).to_string()),
                    ("name", &skill.name),
                    ("unique_name", &skill.unique_name),
                ],
            ));
        }
        println!("{}", lines.join("\n"));
        print!("{}", i18n::t_str("cli.skill_config.prompt"));
        let _ = std::io::stdout().flush();
        let line = read_line().unwrap_or_default();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if trimmed == "a" {
            for skill in &skills {
                let _ = set_enabled(&skill.path, true);
            }
            continue;
        }
        if trimmed == "n" {
            for skill in &skills {
                let _ = set_enabled(&skill.path, false);
            }
            continue;
        }
        if let Ok(index) = trimmed.parse::<usize>()
            && let Some(skill) = skills.get(index.saturating_sub(1))
        {
            let _ = set_enabled(&skill.path, !skill.is_enabled);
        }
    }
}

/// Handle an inline `/command` typed at the menu prompt.
/// 处理在菜单提示符输入的内置 `/命令`。
fn handle_inline_command(
    config: &mut Config,
    registry: &FormatRegistry,
    root: &Path,
    session: &mut SessionLog,
    line: &str,
) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["/ws"] => copy_system_prompt(config, root, registry),
        ["/tools"] => {
            let list = manualaid_ws::prompt::render_tools_list(config, registry);
            let _ = crate::pager::print_paged(&list);
        }
        ["/c"] => copy_round_result(session),
        ["/c", index] => {
            if let Some(index) = parse_round_index(index, session.len()) {
                let results = &session.latest(index).expect("validated index").results;
                match manualaid_core::clipboard::write_clipboard(
                    manualaid_ws::prompt::format_results(results),
                ) {
                    Ok(()) => println!(
                        "{}",
                        t_fmt(
                            "cli.message.result_copied",
                            &[("index", &index.to_string())]
                        )
                    ),
                    Err(e) => eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)])),
                }
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &session.len().to_string())]
                    )
                );
            }
        }
        ["/c", "t", tool_name] => {
            if let Some(tool) = ToolKind::from_name(tool_name) {
                match registry.render_tool_call_template(&tool) {
                    Ok(template) => match manualaid_core::clipboard::write_clipboard(&template) {
                        Ok(()) => println!("{}", i18n::t_str("cli.loop.copied")),
                        Err(e) => {
                            eprintln!("{}", t_fmt("cli.error.clipboard_write", &[("error", &e)]))
                        }
                    },
                    Err(e) => eprintln!("{e}"),
                }
            } else {
                println!("Unknown tool `{tool_name}`");
            }
        }
        ["/lang"] => {
            config.lang = cycle_lang(&config.lang);
            i18n::set_locale(&config.lang);
            persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
        }
        ["/lang", index] => {
            const LANGS: [&str; 2] = ["en", "zh-CN"];
            if let Ok(index) = index.parse::<usize>()
                && let Some(lang) = LANGS.get(index.saturating_sub(1))
            {
                config.lang = (*lang).to_string();
                i18n::set_locale(&config.lang);
                persist_and_confirm(config, root, "cli.config.lang_switched", &config.lang);
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &LANGS.len().to_string())]
                    )
                );
            }
        }
        ["/format"] => {
            config.tool_call_format = cycle_format(&config.tool_call_format);
            let _ = apply_format_mode(registry, config);
            persist_and_confirm(
                config,
                root,
                "cli.config.format_switched",
                &config.tool_call_format,
            );
        }
        ["/format", index] => {
            let labels = RegistryMode::all_labels();
            if let Ok(index) = index.parse::<usize>()
                && let Some(label) = labels.get(index.saturating_sub(1))
            {
                config.tool_call_format = (*label).to_string();
                let _ = apply_format_mode(registry, config);
                persist_and_confirm(
                    config,
                    root,
                    "cli.config.format_switched",
                    &config.tool_call_format,
                );
            } else {
                println!(
                    "{}",
                    t_fmt(
                        "cli.error.invalid_index",
                        &[("count", &labels.len().to_string())]
                    )
                );
            }
        }
        _ => println!("{}", i18n::t_str("cli.loop.menu_invalid")),
    }
}

/// The approval preview shown before each queue item.
/// 每个审批队列项展示前的预览文本。
pub fn approval_preview(item: &AuditQueueItem, params: &IndexMap<String, Value>) -> String {
    let header = t_fmt(
        "cli.approval.item",
        &[
            ("tool", &item.tool_name),
            ("param", &item.param_name),
            (
                "reason",
                item.decision.reason().unwrap_or("approval required"),
            ),
        ],
    );
    let mut detail = match item.tool_name.as_str() {
        "edit" => edit_diff_preview(params),
        "write" => write_preview(params),
        "shell" => params
            .get("command")
            .and_then(Value::as_str)
            .map(|command| format!("$ {command}"))
            .unwrap_or_default(),
        _ => params
            .get(&item.param_name)
            .map(Value::to_string)
            .unwrap_or_default(),
    };
    // Show the AI-supplied purpose (`description`) so the user can judge the
    // operation without expanding the raw command.
    // 展示 AI 提供的调用目的（`description`），让用户无需展开原始命令即可
    // 判断操作。
    if let Some(description) = params.get("description").and_then(Value::as_str)
        && !description.trim().is_empty()
    {
        if !detail.is_empty() {
            detail.push('\n');
        }
        detail.push_str(&t_fmt(
            "cli.approval.description",
            &[("description", description)],
        ));
    }
    if detail.trim().is_empty() {
        header
    } else {
        format!("{header}\n{detail}")
    }
}

/// Build a colored unified diff for an `edit` approval preview, falling
/// back to a `-`/`+` block when the file cannot be read or the replacement
/// would not change anything.
/// 为 `edit` 审批预览构建彩色 unified diff；文件不可读或替换不产生变化
/// 时回退为 `-`/`+` 块。
fn edit_diff_preview(params: &IndexMap<String, Value>) -> String {
    let (Some(file_path), Some(old_string), Some(new_string)) = (
        params.get("file_path").and_then(Value::as_str),
        params.get("old_string").and_then(Value::as_str),
        params.get("new_string").and_then(Value::as_str),
    ) else {
        return String::new();
    };
    let replace_all = params
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let fallback = || colorize_diff(&format!("- {old_string}\n+ {new_string}"));
    match std::fs::read_to_string(file_path) {
        Ok(original) => {
            let modified = if replace_all {
                original.replace(old_string, new_string)
            } else {
                original.replacen(old_string, new_string, 1)
            };
            if modified == original {
                return fallback();
            }
            unified_diff(file_path, &original, &modified)
        }
        Err(_) => fallback(),
    }
}

/// Build a colored preview for a `write` approval: target info plus either
/// a capped diff against existing content or a capped content preview.
/// 为 `write` 审批构建预览：目标信息加上对已有内容的截断 diff，或
/// 不存在时的截断内容预览。
fn write_preview(params: &IndexMap<String, Value>) -> String {
    let Some(file_path) = params.get("file_path").and_then(Value::as_str) else {
        return String::new();
    };
    let content = match params.get("content") {
        Some(Value::String(content)) => content.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    };
    let total = content.lines().count();
    let mut out = format!("write {file_path} ({total} lines, {} bytes)", content.len());
    match std::fs::read_to_string(file_path) {
        Ok(original) => {
            let diff = unified_diff(file_path, &original, &content);
            if diff.is_empty() {
                out.push_str("\ncontent unchanged");
            } else {
                const MAX_DIFF_LINES: usize = 40;
                let lines: Vec<&str> = diff.lines().take(MAX_DIFF_LINES).collect();
                out.push('\n');
                out.push_str(&colorize_diff(&lines.join("\n")));
                if diff.lines().count() > MAX_DIFF_LINES {
                    out.push_str(&format!(
                        "\n... ({} more diff lines)",
                        diff.lines().count() - MAX_DIFF_LINES
                    ));
                }
            }
        }
        Err(_) => {
            if !content.is_empty() {
                const MAX_PREVIEW_LINES: usize = 40;
                let lines: Vec<&str> = content.lines().take(MAX_PREVIEW_LINES).collect();
                out.push('\n');
                out.push_str(&lines.join("\n"));
                if total > MAX_PREVIEW_LINES {
                    out.push_str(&format!("\n... ({} more lines)", total - MAX_PREVIEW_LINES));
                }
            }
        }
    }
    out
}

/// Produce a unified diff between two texts with `a/`/`b/` headers.
/// 生成两个文本之间带 `a/`/`b/` 头的 unified diff。
fn unified_diff(path: &str, original: &str, modified: &str) -> String {
    similar::TextDiff::from_lines(original, modified)
        .unified_diff()
        .header(format!("a/{path}").as_str(), format!("b/{path}").as_str())
        .to_string()
}

/// Color a unified diff line-by-line, only when ANSI styling is enabled.
/// 逐行给 unified diff 着色；仅当 ANSI 样式启用时生效。
fn colorize_diff(diff: &str) -> String {
    let mut out = String::new();
    for line in diff.lines() {
        let styled =
            if line.starts_with("@@") || line.starts_with("--- ") || line.starts_with("+++ ") {
                crate::style::cyan(line)
            } else if line.starts_with('-') {
                crate::style::red(line)
            } else if line.starts_with('+') {
                crate::style::green(line)
            } else if line.starts_with(' ') {
                crate::style::gray(line)
            } else {
                line.to_string()
            };
        out.push_str(&styled);
        out.push('\n');
    }
    out
}

/// Clear the screen via the platform command; failures are ignored.
/// 通过平台命令清屏；失败时静默忽略。
fn clear_screen() {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "cls"])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("clear")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }
}

/// Read one line from stdin; EOF or an error yields `None`.
/// 从标准输入读取一行；EOF 或出错返回 `None`。
fn read_line() -> Option<String> {
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line),
        Err(_) => None,
    }
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

    #[test]
    fn parse_round_index_defaults_to_latest() {
        assert_eq!(parse_round_index("", 3), Some(1));
        assert_eq!(parse_round_index("2", 3), Some(2));
        assert_eq!(parse_round_index("0", 3), None);
        assert_eq!(parse_round_index("4", 3), None);
        assert_eq!(parse_round_index("abc", 3), None);
    }

    #[test]
    fn cycle_lang_switches_between_two_locales() {
        assert_eq!(cycle_lang("en"), "zh-CN");
        assert_eq!(cycle_lang("zh-CN"), "en");
    }

    #[test]
    fn cycle_format_wraps_around() {
        assert_eq!(cycle_format("auto"), "xml");
        assert_eq!(cycle_format("xml"), "json-codeblock");
        assert_eq!(cycle_format("json-codeblock"), "auto");
        assert_eq!(cycle_format("bogus"), "xml");
    }

    #[test]
    fn format_round_summary_shows_state_and_output() {
        i18n::set_locale("en");
        let results = vec![
            ToolResult::success("read", "hello", true),
            ToolResult::failure("edit", "boom"),
        ];
        let summary = format_round_summary(&results);
        assert!(summary.contains("[read] success"));
        assert!(summary.contains("hello"));
        assert!(summary.contains("[edit] failure"));
        assert!(summary.contains("boom"));
    }

    #[test]
    fn approval_preview_shows_shell_command() {
        let item = AuditQueueItem {
            tool_name: "shell".into(),
            param_name: "command".into(),
            decision: AuditDecision::NeedsApproval("reason".into()),
        };
        let mut params = IndexMap::new();
        params.insert("command".to_string(), Value::String("git status".into()));
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("$ git status"));
        assert!(preview.contains("reason"));
    }

    #[test]
    fn approval_preview_includes_ai_description() {
        let item = AuditQueueItem {
            tool_name: "shell".into(),
            param_name: "command".into(),
            decision: AuditDecision::NeedsApproval("reason".into()),
        };
        let mut params = IndexMap::new();
        params.insert("command".to_string(), Value::String("git status".into()));
        params.insert(
            "description".to_string(),
            Value::String("check the repo state".into()),
        );
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("$ git status"));
        assert!(preview.contains("check the repo state"));
    }

    #[test]
    fn apply_cli_lang_overrides_config_and_ignores_invalid() {
        let mut config = Config::default();
        apply_cli_lang(Some("zh-CN".to_string()), &mut config);
        assert_eq!(config.lang, "zh-CN");
        apply_cli_lang(Some("fr".to_string()), &mut config);
        assert_eq!(config.lang, "zh-CN");
        apply_cli_lang(None, &mut config);
        assert_eq!(config.lang, "zh-CN");
    }

    #[test]
    fn approval_preview_raw_value_for_other_tools() {
        let item = AuditQueueItem {
            tool_name: "read".into(),
            param_name: "file_path".into(),
            decision: AuditDecision::NeedsApproval("outside".into()),
        };
        let mut params = IndexMap::new();
        params.insert("file_path".to_string(), Value::String("/etc/passwd".into()));
        let preview = approval_preview(&item, &params);
        assert!(preview.contains("/etc/passwd"));
    }

    #[test]
    fn colorize_diff_keeps_plain_text_without_style() {
        crate::style::set_enabled(false);
        let colored = colorize_diff("@@ -1 +1 @@\n-a\n+b\n");
        assert!(!colored.contains("\x1b["));
        crate::style::set_enabled(true);
        let styled = colorize_diff("@@ -1 +1 @@\n-a\n+b\n");
        assert!(styled.contains("\x1b["));
        crate::style::set_enabled(false);
    }
}
