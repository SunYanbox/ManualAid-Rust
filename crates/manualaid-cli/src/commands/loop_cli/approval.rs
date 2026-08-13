//! Per-round approval flow: parse, audit, ask and execute.
//! 单轮审批流程：解析、审计、询问并执行。

use std::time::Duration;

use manualaid_core::audit::{AuditDecision, AuditQueueItem};
use manualaid_core::executor::Executor;
use manualaid_core::parser::{FormatRegistry, ParsedToolCall};
use manualaid_core::tools::{ToolResult, params_summary_of};
use manualaid_ws::session::RoundStats;

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
/// become failure results. Returns the parsed calls, results and round
/// statistics (parse/audit/execution durations and estimated tokens).
/// 解析并执行一轮带用户审批的工具调用。
///
/// 每个调用都会先经过预检：必然失败的调用直接产生失败结果。其余调用
/// 进入审计，需要批准的项目逐条分页展示，随后短暂停顿再询问；`decide`
/// 返回用户对每一项的答复。已批准的调用正常执行，被拒绝的调用生成失败
/// 结果。返回本轮解析出的调用、结果与统计信息（解析/审计/执行耗时与
/// 估算 Token）。
pub async fn execute_round_with_approval(
    executor: &Executor,
    registry: &FormatRegistry,
    input: &str,
    mut decide: impl FnMut(&AuditQueueItem) -> Approval,
) -> Result<(Vec<ParsedToolCall>, Vec<ToolResult>, RoundStats), String> {
    let parse_start = std::time::Instant::now();
    let outcome = registry
        .parse(input)
        .map_err(|e| t_fmt("cli.error.parse", &[("error", &e.to_string())]))?;
    let parse_duration = parse_start.elapsed();
    // 解析器产生的软警告（如被丢弃的未闭合参数）直接展示给用户。
    for warning in &outcome.warnings {
        crate::console::out_println!("⚠ {warning}");
    }
    let calls = outcome.calls;
    if calls.is_empty() {
        return Err(i18n::t_str("cli.error.no_calls"));
    }
    let parsed_calls = calls.clone();

    // Audit duration spans pre-checks and the approval queue; execution
    // time is measured separately per tool by the executor.
    // 审计耗时覆盖预检与审批队列；执行时间由执行器按工具单独测量。
    let audit_start = std::time::Instant::now();

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
        crate::console::out_println!(
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

    let audit_duration = audit_start.elapsed();

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

    let total_tokens = estimate_round_tokens(input, &mut results);
    let stats = RoundStats {
        parse_duration_ms: parse_duration.as_millis() as u64,
        audit_duration_ms: audit_duration.as_millis() as u64,
        total_execution_duration_ms: results.iter().map(|r| r.execution_duration_ms).sum(),
        total_tokens,
    };
    Ok((parsed_calls, results, stats))
}

/// Estimate the token consumption of one round and attach the per-call
/// share to each result. The input text (as the model produced it) is
/// counted once; each result's XML-wrapped output (as the model will
/// receive it) is counted individually. The per-call estimate apportions
/// the input tokens equally across calls, plus the call's own output.
/// 估算一轮的 Token 消耗并把每次调用的分摊额写入结果。输入文本（模型
/// 产出原文）计一次；每个结果的 XML 包裹输出（模型将收到的形式）单独
/// 计数。单调用估算 = 输入 Token 均摊到各调用 + 该调用自身输出。
fn estimate_round_tokens(input: &str, results: &mut [ToolResult]) -> u64 {
    let input_tokens = tokenx_rs::estimate_token_count(input);
    let num_calls = results.len().max(1);
    let mut total = input_tokens as u64;
    for result in results {
        // usize::MAX never truncates and skips the i18n truncation branch.
        // usize::MAX 永不截断，且不会进入 i18n 截断分支。
        let wrapped =
            manualaid_ws::prompt::format_results(std::slice::from_ref(result), usize::MAX);
        let output_tokens = tokenx_rs::estimate_token_count(&wrapped) as u64;
        result.estimated_tokens = input_tokens as u64 / num_calls as u64 + output_tokens;
        total += output_tokens;
    }
    total
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
    crate::console::out_print!(
        "{}",
        t_fmt(
            "cli.audit.prompt",
            &[("tool", &item.tool_name), ("param", &item.param_name),],
        )
    );
    crate::console::flush();
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
    use manualaid_core::audit::{Auditor, SessionMode};
    use manualaid_core::executor::Executor;
    use manualaid_core::parser::FormatRegistry;
    use std::sync::Arc;

    fn queue_item() -> AuditQueueItem {
        AuditQueueItem {
            tool_name: "read".to_string(),
            param_name: "file_path".to_string(),
            decision: AuditDecision::NeedsApproval("outside".to_string()),
        }
    }

    #[test]
    fn ask_approval_maps_answers() {
        let _capture = crate::console::capture();
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

    #[tokio::test]
    async fn execute_round_prints_parser_warnings() {
        let capture = crate::console::capture();
        let executor = Executor::new(
            Auditor::new(std::env::temp_dir()).with_mode(SessionMode::AcceptEdit),
            Arc::new(None),
        );
        let registry = FormatRegistry::new();
        // 未闭合参数被解析器丢弃并产生软警告，警告应打印到控制台。
        let input = "<edit><old_string>x</edit>";
        let (calls, results, _stats) =
            execute_round_with_approval(&executor, &registry, input, |_| Approval::Approve)
                .await
                .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(results.len(), 1);
        let text = capture.text();
        assert!(text.contains('⚠'), "parser warning not printed: {text}");
        assert!(text.contains("old_string"));
    }

    fn parsed_call() -> ParsedToolCall {
        FormatRegistry::new()
            .parse("<read><file_path>Z:/a.txt</file_path></read>")
            .unwrap()
            .calls
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

    #[test]
    fn estimate_round_tokens_apportions_input_and_sums_outputs() {
        let input =
            "<read><file_path>a.txt</file_path></read>\n<read><file_path>b.txt</file_path></read>";
        let mut results = vec![
            ToolResult::success("read", "hello world", true),
            ToolResult::success("read", "another output", true),
        ];
        let input_tokens = tokenx_rs::estimate_token_count(input) as u64;
        let expected_total = input_tokens
            + results
                .iter()
                .map(|r| {
                    tokenx_rs::estimate_token_count(&manualaid_ws::prompt::format_results(
                        std::slice::from_ref(r),
                        usize::MAX,
                    )) as u64
                })
                .sum::<u64>();
        let total = estimate_round_tokens(input, &mut results);
        assert_eq!(total, expected_total);
        // The round total counts the input text exactly once.
        // 轮总数中输入文本恰好计一次。
        assert!(total >= input_tokens);
        for result in &results {
            let own_output = tokenx_rs::estimate_token_count(&manualaid_ws::prompt::format_results(
                std::slice::from_ref(result),
                usize::MAX,
            )) as u64;
            assert_eq!(result.estimated_tokens, input_tokens / 2 + own_output);
        }
    }
}
