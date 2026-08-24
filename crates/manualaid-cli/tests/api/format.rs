//! API tests: output formatting and paging.
//! API 测试：输出格式化与分页。

use std::time::Duration;

use manualaid_cli::{
    format_bytes, format_default_output, format_duration, format_error_output, format_mask_output,
    format_restore_output, format_timings, pager, style, t_fmt,
};

#[test]
fn t_fmt_replaces_placeholders() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let name_line = t_fmt("cli.skill.unique_name", &[("unique_name", "u")]);
    assert!(name_line.contains("Unique name: u"));
    let chars_line = t_fmt("cli.skill.desc_chars_total", &[("chars", "42")]);
    assert!(chars_line.contains("Total chars: 42"));
    assert!(!name_line.contains("%{"));
    assert!(!chars_line.contains("%{"));
}

#[test]
fn t_fmt_leaves_unknown_placeholders_untouched() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    let line = t_fmt("cli.skill.unique_name", &[("name", "n")]);
    assert!(line.contains("%{unique_name}"));
    assert!(!line.contains("%{name}"));
}

#[test]
fn format_default_output_plain_and_styled() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(
        format_default_output("ManualAid running..."),
        "ManualAid running...\n"
    );
    style::set_enabled(true);
    assert_eq!(
        format_default_output("ManualAid running..."),
        "\n\x1b[32mManualAid running...\x1b[0m\n"
    );
}

#[test]
fn format_mask_output_has_two_headed_sections() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    let out = format_mask_output("contact [PRV_EMAIL_1]", "{\n  \"k\": \"v\"\n}");
    assert!(out.starts_with("\nMasked text\ncontact [PRV_EMAIL_1]\n\nSnapshot JSON\n"));
    assert!(out.ends_with("\"v\"\n}\n"));
    style::set_enabled(true);
    let out = format_mask_output("m", "{}");
    assert!(out.contains("\x1b[1;36mMasked text\x1b[0m\nm"));
    assert!(out.contains("\x1b[1;36mSnapshot JSON\x1b[0m\n{}"));
}

#[test]
fn format_restore_output_plain_and_styled() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(format_restore_output("hello"), "hello\n");
    assert_eq!(format_restore_output(""), "");
    style::set_enabled(true);
    assert_eq!(
        format_restore_output("hello"),
        "\n\x1b[1;36mRestored text\x1b[0m\n\x1b[32mhello\x1b[0m\n"
    );
    assert_eq!(format_restore_output(""), "");
}

#[test]
fn format_error_output_plain_and_styled() {
    let _guard = super::locale_guard();
    let _style = super::style_guard();
    i18n::set_locale("en");
    style::set_enabled(false);
    assert_eq!(
        format_error_output("Masking failed: x"),
        "Masking failed: x\n"
    );
    style::set_enabled(true);
    assert_eq!(
        format_error_output("Masking failed: x"),
        "\x1b[1;31mError: Masking failed: x\x1b[0m\n"
    );
}

#[test]
fn pager_prints_all_when_not_terminal() {
    let _capture = manualaid_cli::console::capture();
    pager::set_enabled(false);
    pager::print_paged("line one\nline two\n").expect("print should succeed");
}

#[test]
fn collapsed_pager_prints_all_when_not_terminal() {
    let _capture = manualaid_cli::console::capture();
    pager::set_enabled(false);
    let long = (1..=100)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    pager::print_paged_three_lines(&long).expect("print should succeed");
}

#[test]
fn format_duration_is_milliseconds_with_nanosecond_precision() {
    assert_eq!(format_duration(Duration::ZERO), "0.000000 ms");
    assert_eq!(format_duration(Duration::from_nanos(5)), "0.000005 ms");
    assert_eq!(
        format_duration(Duration::from_nanos(1_234_567)),
        "1.234567 ms"
    );
    assert_eq!(format_duration(Duration::from_secs(2)), "2000.000000 ms");
}

#[test]
fn format_bytes_auto_selects_unit_with_three_decimals() {
    assert_eq!(format_bytes(0), "0.000 KB");
    assert_eq!(format_bytes(512), "0.500 KB");
    assert_eq!(format_bytes(1536), "1.500 KB");
    assert_eq!(format_bytes(1_572_864), "1.500 MB");
    assert_eq!(format_bytes(1_073_741_824), "1.000 GB");
}

#[test]
fn format_timings_renders_heading_and_lines() {
    let _guard = super::locale_guard();
    i18n::set_locale("en");
    assert_eq!(format_timings(&[]), "");
    let out = format_timings(&[
        "Mask: 1.234567 ms (25 chars)".to_string(),
        "Init: 0.000123 ms".to_string(),
    ]);
    assert!(
        out.starts_with("\nTimings\n  - Mask: 1.234567 ms (25 chars)\n  - Init: 0.000123 ms\n")
    );
    assert!(out.ends_with('\n'));
}
