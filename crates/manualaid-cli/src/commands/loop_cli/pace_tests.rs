//! Tests for the copy-pace meter.
//! 复制节奏计量的测试。

use super::*;

/// A ledger holding one write per given second offset, counted as tool
/// results.
/// 一个 ledger：按给定的秒偏移各写入一次，全部计为工具结果。
fn ledger_at_offsets(offsets: &[u64]) -> (CopyLedger, Instant) {
    let base = Instant::now();
    let mut ledger = CopyLedger::default();
    for offset in offsets {
        ledger.record(CopyKind::Result, base + Duration::from_secs(*offset));
    }
    (ledger, base)
}

#[test]
fn the_first_copy_has_no_gap_and_the_second_reports_one() {
    let (ledger, base) = ledger_at_offsets(&[0, 78]);
    let report = ledger.report(base + Duration::from_secs(78));
    assert_eq!(report.gap, Some(Duration::from_secs(78)));
    assert_eq!(report.last_at, Some(base + Duration::from_secs(78)));

    let mut first_only = CopyLedger::default();
    first_only.record(CopyKind::Prompt, base);
    let report = first_only.report(base);
    assert_eq!(report.gap, None);
    assert_eq!(report.last_at, Some(base));
}

#[test]
fn windows_count_only_the_copies_inside_each_one() {
    // Copies at 50min, 30min, 9min, 4min and 30s before the reading at
    // 50min, so the windows hold 1, 2, 3, 4 and 5 copies respectively.
    // 复制发生在此次读数前 50min、30min、9min、4min 与 30s，各窗口因此分别
    // 为 1、2、3、4、5 次。
    let offsets = [0, 30 * 60, 41 * 60, 46 * 60, 50 * 60 - 30];
    let (ledger, base) = ledger_at_offsets(&offsets);
    let report = ledger.report(base + Duration::from_secs(50 * 60));
    assert_eq!(report.windows, [1, 2, 3, 4, 5]);
}

#[test]
fn a_copy_older_than_one_hour_leaves_the_widest_window() {
    let base = Instant::now();
    let mut ledger = CopyLedger::default();
    ledger.record(CopyKind::Result, base);
    ledger.record(CopyKind::Result, base + Duration::from_secs(60 * 61));
    let report = ledger.report(base + Duration::from_secs(60 * 61));
    assert_eq!(report.windows, [1, 1, 1, 1, 1]);
}

#[test]
fn tool_templates_are_not_counted_but_prompts_and_results_are() {
    let base = Instant::now();
    let mut ledger = CopyLedger::default();
    ledger.record(CopyKind::Template, base);
    ledger.record(CopyKind::SystemPrompt, base);
    ledger.record(CopyKind::Prompt, base);
    ledger.record(CopyKind::Result, base);
    let report = ledger.report(base);
    assert_eq!(report.system_prompt, 1);
    assert_eq!(report.snippet, 1);
    assert_eq!(report.result, 1);
    assert_eq!(report.send_copies(), 3);
    assert_eq!(report.windows, [3, 3, 3, 3, 3]);
}

#[test]
fn the_ledger_keeps_at_most_the_widest_window_of_writes() {
    let base = Instant::now();
    let mut ledger = CopyLedger::default();
    // Two writes an hour apart: the older one is pruned by the second.
    // 相隔一小时的两次写入：较旧的一次被第二次写入清理掉。
    ledger.record(CopyKind::Result, base);
    ledger.record(CopyKind::Result, base + Duration::from_secs(2 * 60 * 60));
    assert_eq!(ledger.writes.len(), 1);
    // Lifetime counters survive pruning.
    // 会话累计计数不受清理影响。
    assert_eq!(ledger.report(base).result, 2);
}

#[test]
fn the_paced_provider_records_successful_writes_only() {
    let ledger = Arc::new(Mutex::new(CopyLedger::default()));
    let provider = PacedClipboard::new(MockClipboard::new(), Arc::clone(&ledger));
    provider
        .write_kind(CopyKind::SystemPrompt, "system prompt")
        .unwrap();

    let report = provider.pace();
    assert_eq!(report.system_prompt, 1);
    assert_eq!(report.send_copies(), 1);

    // A failed write posts nothing, so it is not counted.
    // 写入失败没有发出任何东西，因此不计入。
    let failing_clipboard = MockClipboard::new();
    failing_clipboard.set_write_error("mock write failure");
    let failing = PacedClipboard::new(failing_clipboard, Arc::clone(&ledger));
    assert!(failing.write_kind(CopyKind::Result, "results").is_err());
    assert_eq!(ledger.lock().unwrap().report(Instant::now()).result, 0);
}

#[test]
fn the_paced_provider_reads_through_to_the_wrapped_clipboard() {
    let inner = MockClipboard::new();
    inner.write("existing").unwrap();
    let provider = PacedClipboard::new(inner, Arc::new(Mutex::new(CopyLedger::default())));
    assert_eq!(provider.read().unwrap(), "existing");
}

#[test]
fn a_plain_write_is_counted_as_a_prompt_snippet() {
    let ledger = Arc::new(Mutex::new(CopyLedger::default()));
    let provider = PacedClipboard::new(MockClipboard::new(), Arc::clone(&ledger));
    provider.write("unlabelled").unwrap();
    let report = provider.pace();
    assert_eq!(report.snippet, 1);
    assert_eq!(report.result, 0);
}

#[test]
fn providers_without_metering_report_an_empty_reading() {
    let real = RealClipboard.pace();
    assert_eq!(real.last_at, None);
    assert_eq!(real.gap, None);
    assert_eq!(real.windows, [0; WINDOWS.len()]);
    assert_eq!(real.send_copies(), 0);
    assert_eq!(MockClipboard::new().pace().send_copies(), 0);
}

#[test]
fn pace_lines_fold_the_long_windows_and_expand_them_on_request() {
    let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let offsets = [0, 30 * 60, 41 * 60, 46 * 60, 50 * 60 - 30];
    let (ledger, base) = ledger_at_offsets(&offsets);
    let report = ledger.report(base + Duration::from_secs(50 * 60));

    let folded = pace_lines(&report, false, false);
    assert_eq!(folded.len(), 2);
    // The gap spans the last two copies: 46min to 49min30s.
    // 间隔为最后两次复制之间：46min 到 49min30s。
    assert!(folded[0].contains("3m30s"), "gap line: {}", folded[0]);
    assert!(folded[1].contains("Last 60s: 1"));
    assert!(folded[1].contains("Last 10min: 3"));
    assert!(!folded[1].contains("Last 30min"));

    let expanded = pace_lines(&report, true, false);
    assert_eq!(expanded.len(), 2);
    assert!(expanded[1].contains("Last 30min: 4"));
    assert!(expanded[1].contains("Last 60min: 5"));
}

#[test]
fn pace_lines_show_the_fold_hint_only_when_asked() {
    let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let (ledger, base) = ledger_at_offsets(&[0, 10]);
    let report = ledger.report(base + Duration::from_secs(10));

    let with_hint = pace_lines(&report, false, true);
    assert_eq!(with_hint.len(), 3);
    assert!(with_hint[2].contains("configuration menu"));
    let without_hint = pace_lines(&report, false, false);
    assert_eq!(without_hint.len(), 2);
}

#[test]
fn pace_lines_omit_the_gap_line_for_the_first_copy() {
    let _lock = crate::test_support::LOCALE_LOCK.lock().unwrap();
    i18n::set_locale("en");
    let (ledger, base) = ledger_at_offsets(&[0]);
    let lines = pace_lines(&ledger.report(base), false, false);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("Last 60s: 1"));
}
