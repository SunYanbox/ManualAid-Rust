//! Per-round approval flow: parse, audit, ask and execute.
//! 单轮审批流程：解析、审计、询问并执行。

use std::io::Write;

use manualaid_core::audit::{AuditDecision, AuditQueueItem};
use manualaid_core::executor::Executor;
use manualaid_core::parser::{FormatRegistry, ParsedToolCall};
use manualaid_core::tools::{ToolResult, params_summary_of};

use super::Approval;
use super::preview::approval_preview;
use super::utils::{read_line, t_fmt};

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
pub(super) fn denied_result(
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

/// Ask the user whether to approve one operation (`y` / `n` / `t`).
/// 询问用户是否批准某一操作（`y` / `n` / `t`）。
pub(super) fn ask_approval(item: &AuditQueueItem) -> Approval {
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
