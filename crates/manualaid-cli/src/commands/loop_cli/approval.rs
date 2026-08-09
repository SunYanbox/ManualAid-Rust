//! Per-round approval flow: parse, audit, ask and execute.
//! 单轮审批流程：解析、审计、询问并执行。

use std::io::Write;
use std::time::Duration;

use manualaid_core::audit::{AuditDecision, AuditQueueItem};
use manualaid_core::executor::Executor;
use manualaid_core::parser::{FormatRegistry, ParsedToolCall};
use manualaid_core::tools::{ToolResult, params_summary_of};

use super::Approval;
use super::preview::{approval_preview, edit_diff_preview, write_preview};
use super::utils::{read_line, t_fmt};

/// Delay between the preview output and the approval prompt, so paged
/// output never streams into the answer that the caller consumes.
/// 预览输出与审批提问之间的停顿，避免分页输出流入调用方读取的答复。
const APPROVAL_PAUSE: Duration = Duration::from_millis(500);

/// Parse and execute one round of tool calls with user approval.
///
/// Every call is pre-checked first: calls guaranteed to fail produce a
/// failure result directly. Remaining calls are audited and the ones
/// needing approval are presented one by one; `decide` returns the user's
/// answer for each item. Each preview is paged, followed by a short pause
/// before the approval question. Approved calls execute, denied calls
/// become failure results. Returns the parsed calls and results of the round.
/// 解析并执行一轮带用户审批的工具调用。
///
/// 每个调用都会先经过预检：必然失败的调用直接产生失败结果。其余调用
/// 进入审计，需要批准的项目逐条分页展示，随后短暂停顿再询问；`decide`
/// 返回用户对每一项的答复。已批准的调用正常执行，被拒绝的调用生成失败
/// 结果。返回本轮解析出的调用与结果。
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
                let preview = approval_preview(&queue_item, &item.call.params);
                let _ = crate::pager::print_paged_collapsed(&preview);
                tokio::time::sleep(APPROVAL_PAUSE).await;
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
        // Auto-approved calls (AcceptEdit) skip the approval preview, so
        // capture the edit/write diff beforehand and show it after a
        // successful execution to confirm what changed.
        // 自动放行的调用（AcceptEdit）不会展示审批预览；执行前捕获
        // edit/write 的 diff，成功执行后再展示，便于确认实际改动。
        let executed_diff = if item.pending.is_empty() {
            match item.call.tool_name.as_str() {
                "edit" => Some(edit_diff_preview(&item.call.params)),
                "write" => Some(write_preview(&item.call.params)),
                _ => None,
            }
        } else {
            None
        };
        let mut result = executor.execute(item.call).await;
        if result.success
            && let Some(diff) = executed_diff
            && !diff.trim().is_empty()
        {
            let _ = crate::pager::print_paged_collapsed(&diff);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::loop_cli::utils::push_test_input;
    use manualaid_core::parser::FormatRegistry;

    fn queue_item() -> AuditQueueItem {
        AuditQueueItem {
            tool_name: "read".to_string(),
            param_name: "file_path".to_string(),
            decision: AuditDecision::NeedsApproval("outside".to_string()),
        }
    }

    #[test]
    fn ask_approval_maps_answers() {
        let item = queue_item();
        push_test_input(&["y"]);
        assert_eq!(ask_approval(&item), Approval::Approve);
        push_test_input(&["n"]);
        assert_eq!(ask_approval(&item), Approval::Deny);
        push_test_input(&["t", "use another tool"]);
        assert_eq!(
            ask_approval(&item),
            Approval::DenyWithText("use another tool".to_string())
        );
        push_test_input(&["t"]);
        assert_eq!(ask_approval(&item), Approval::DenyWithText(String::new()));
        push_test_input(&[""]);
        assert_eq!(ask_approval(&item), Approval::Deny);
    }

    #[test]
    fn approval_pause_is_half_a_second() {
        assert_eq!(APPROVAL_PAUSE, Duration::from_millis(500));
    }

    fn parsed_call() -> ParsedToolCall {
        FormatRegistry::new()
            .parse("<read><file_path>Z:/a.txt</file_path></read>")
            .unwrap()
            .remove(0)
    }

    #[test]
    fn denied_result_prefers_typed_text() {
        let result = denied_result(&parsed_call(), None, Some("use the other tool".into()));
        assert!(!result.success);
        assert_eq!(result.output, "use the other tool");
    }

    #[test]
    fn denied_result_uses_decision_reason() {
        let call = parsed_call();
        let needs = denied_result(
            &call,
            Some(&AuditDecision::NeedsApproval("blocked".into())),
            None,
        );
        assert!(needs.output.contains("blocked"));
        let denied = denied_result(&call, Some(&AuditDecision::Denied("rejected".into())), None);
        assert!(denied.output.contains("rejected"));
    }

    #[test]
    fn denied_result_falls_back_to_default_message() {
        let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let result = denied_result(&parsed_call(), None, None);
        assert!(result.output.contains("denied"));
    }
}
