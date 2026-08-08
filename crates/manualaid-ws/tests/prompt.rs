//! Integration tests for system-prompt building and result formatting.
//! 系统提示词构建与结果格式化的集成测试。

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::Skill;
use manualaid_core::tools::ToolResult;
use manualaid_ws::config::Config;
use manualaid_ws::prompt::{build_system_prompt, format_results, render_tools_list};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `rust_i18n` keeps one process-wide locale, so every test that asserts
/// localized text must run under this lock to avoid races.
/// `rust_i18n` 的 locale 是进程级全局状态，所有断言本地化文本的测试必须
/// 在同一个锁下运行，避免并行竞态。
static LANG_LOCK: Mutex<()> = Mutex::new(());

const MAX: usize = 50_000;

/// Run `f` with `lang` active, then restore English for other tests.
/// A poisoned lock is recovered so one failing test does not cascade.
/// 在 `lang` 语言下执行 `f`，结束后恢复英文供其他测试使用。
/// 锁被污染时直接接管，避免单个测试失败引发级联失败。
fn with_locale(lang: &str, f: impl FnOnce()) {
    let _guard = LANG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    i18n::set_locale(lang);
    f();
    i18n::set_locale("en");
}

#[test]
fn tools_list_uses_localized_descriptions() {
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let list = render_tools_list(&Config::default(), &registry);
        assert!(list.contains("## read"));
        assert!(list.contains("absolute path"));
    });
    with_locale("zh-CN", || {
        let registry = FormatRegistry::new();
        let list = render_tools_list(&Config::default(), &registry);
        assert!(list.contains("绝对路径"));
    });
}

#[test]
fn system_prompt_reflects_config_switches() {
    with_locale("en", || {
        let config = Config {
            skill: false,
            ..Config::default()
        };
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[]);
        assert!(prompt.contains("<system_prompt>"));
        assert!(prompt.contains("C:/ws"));
        assert!(!prompt.contains("<skill-usage>"));
    });
}

#[test]
fn system_prompt_drops_skill_when_none_are_enabled() {
    with_locale("en", || {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[]);
        assert!(!prompt.contains("<skill-usage>"));
        assert!(!prompt.contains("## skill"));
        assert!(!prompt.contains("<available_skills>"));
    });
}

#[test]
fn system_prompt_includes_enabled_skills() {
    with_locale("en", || {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let skill = Skill {
            unique_name: "demo".to_string(),
            name: "demo".to_string(),
            description: "demo skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/demo"),
            is_global: false,
            is_enabled: true,
        };
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[skill]);
        assert!(prompt.contains("<skill-usage>"));
        assert!(prompt.contains("## skill"));
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("demo"));
    });
}

#[test]
fn system_prompt_skips_disabled_skills_in_list() {
    with_locale("en", || {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let enabled = Skill {
            unique_name: "demo".to_string(),
            name: "demo".to_string(),
            description: "demo skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/demo"),
            is_global: false,
            is_enabled: true,
        };
        let disabled = Skill {
            unique_name: "hidden".to_string(),
            name: "hidden".to_string(),
            description: "hidden skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/hidden"),
            is_global: false,
            is_enabled: false,
        };
        let prompt =
            build_system_prompt(&config, Path::new("C:/ws"), &registry, &[enabled, disabled]);
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("demo"));
        assert!(!prompt.contains("hidden"));
    });
}

#[test]
fn format_results_joins_multiple_results() {
    let results = vec![
        ToolResult::success("read", "a", true),
        ToolResult::failure("edit", "b"),
    ];
    let text = format_results(&results, MAX);
    assert_eq!(text.matches("<tool_result").count(), 2);
    assert!(text.contains("success=\"true\""));
    assert!(text.contains("success=\"false\""));
}

#[test]
fn format_results_escapes_attribute_values() {
    let result = ToolResult::success("read", "content", true)
        .with_params_summary("{\"file_path\":\"/a.txt\"}".into());
    let text = format_results(&[result], MAX);
    assert!(text.contains("<tool_result name=\"read\""));
    assert!(text.contains("&quot;file_path&quot;"));
    assert!(text.contains("success=\"true\""));
}

#[test]
fn format_results_omits_empty_summary() {
    let result = ToolResult::success("shell", "done", false);
    let text = format_results(&[result], MAX);
    assert!(text.contains("<tool_result name=\"shell\" success=\"true\">"));
    assert!(!text.contains("params="));
}

#[test]
fn format_results_empty_input_returns_empty_string() {
    assert_eq!(format_results(&[], MAX), "");
}

#[test]
fn format_results_within_limit_is_unchanged() {
    let results = vec![
        ToolResult::success("read", "hello", true),
        ToolResult::failure("edit", "world"),
    ];
    let text = format_results(&results, MAX);
    assert_eq!(
        text,
        "<tool_result name=\"read\" success=\"true\">\nhello\n</tool_result>\n\n\
         <tool_result name=\"edit\" success=\"false\">\nworld\n</tool_result>"
    );
}

#[test]
fn format_results_preserves_whitespace_of_slices() {
    let result = ToolResult::success("read", "    indented\n  second  \n", true);
    let text = format_results(&[result], MAX);
    assert!(text.contains("    indented\n  second  \n\n</tool_result>"));
}

#[test]
fn format_results_truncates_proportionally_with_notices_and_warning() {
    with_locale("en", || {
        let results = vec![
            ToolResult::success("read", "χ".repeat(90_000), true),
            ToolResult::failure("shell", "λ".repeat(30_000)),
        ];
        let text = format_results(&results, 60_000);
        // 90k and 30k out of 120k total get 45k and 15k of the 60k budget.
        // 9 万与 3 万字符按 12 万总量分配 6 万预算，分别得到 4.5 万与 1.5 万。
        assert_eq!(text.matches('χ').count(), 45_000);
        assert_eq!(text.matches('λ').count(), 15_000);
        assert!(text.contains("[Output truncated: 45000 of 90000 chars removed]"));
        assert!(text.contains("[Output truncated: 15000 of 30000 chars removed]"));
        assert!(text.ends_with(
            "Output exceeded 60000 characters (total: 120000). Truncated proportionally. \
             Please adjust tool call parameters to avoid frequently exceeding the character limit."
        ));
    });
}

#[test]
fn format_results_keeps_short_results_whole() {
    with_locale("en", || {
        let results = vec![
            ToolResult::success("read", "χ".repeat(90_000), true),
            ToolResult::failure("shell", "λ".repeat(500)),
        ];
        let text = format_results(&results, 50_000);
        // The 500-char result is below the keep floor and stays untouched.
        // 500 字符的结果低于保底阈值，保持完整。
        assert_eq!(text.matches('λ').count(), 500);
        assert_eq!(text.matches('χ').count(), 49_500);
        assert_eq!(text.matches("[Output truncated:").count(), 1);
        assert!(text.contains("[Output truncated: 40500 of 90000 chars removed]"));
    });
}

#[test]
fn format_results_floor_overshoot_is_taken_from_largest_allocation() {
    with_locale("en", || {
        let results = vec![
            ToolResult::success("read", "χ".repeat(10_000), true),
            ToolResult::failure("shell", "λ".repeat(100_000)),
        ];
        let text = format_results(&results, 10_500);
        // The small result's proportional share is below the 1000-char floor, so
        // it is kept at the floor and the overshoot is cut from the larger one.
        // 小结果的按比例份额低于 1000 字符保底，按保底保留，超出部分从大结果中扣减。
        assert_eq!(text.matches('χ').count(), 1_000);
        assert_eq!(text.matches('λ').count(), 9_500);
        assert!(text.contains("[Output truncated: 9000 of 10000 chars removed]"));
        assert!(text.contains("[Output truncated: 90500 of 100000 chars removed]"));
    });
}

#[test]
fn format_results_all_short_drops_whole_results_from_the_end() {
    with_locale("en", || {
        let results = vec![
            ToolResult::success("read", "χ".repeat(500), true),
            ToolResult::failure("edit", "λ".repeat(500)),
            ToolResult::success("shell", "π".repeat(500), false),
        ];
        let text = format_results(&results, 1_000);
        // Nothing can be shortened, so whole results are dropped from the end.
        // 没有可缩短的结果，从末尾整块丢弃。
        assert_eq!(text.matches("<tool_result").count(), 2);
        assert_eq!(text.matches('χ').count(), 500);
        assert_eq!(text.matches('λ').count(), 500);
        assert_eq!(text.matches('π').count(), 0);
        assert!(text.contains("Output exceeded 1000 characters (total: 1500)"));
    });
}

#[test]
fn format_results_unpayable_overshoot_stays_at_floor() {
    with_locale("en", || {
        let results = vec![
            ToolResult::success("read", "χ".repeat(1_001), true),
            ToolResult::failure("edit", "λ".repeat(1_001)),
            ToolResult::success("shell", "π".repeat(1_001), false),
        ];
        let text = format_results(&results, 1_500);
        // Every result's proportional share falls below the 1000-char floor,
        // so all three sit at the floor and the overshoot stays unpaid.
        // 每个结果的按比例份额都低于 1000 字符保底，三者都停在保底值，
        // 超出预算的部分无法回扣。
        assert_eq!(text.matches('χ').count(), 1_000);
        assert_eq!(text.matches('λ').count(), 1_000);
        assert_eq!(text.matches('π').count(), 1_000);
        assert_eq!(text.matches("[Output truncated:").count(), 3);
        assert!(text.contains("Output exceeded 1500 characters (total: 3003)"));
    });
}

#[test]
fn format_results_notice_is_localized() {
    with_locale("zh-CN", || {
        let results = vec![ToolResult::success("read", "χ".repeat(90_000), true)];
        let text = format_results(&results, 60_000);
        // The single 90k-char result gets the whole 60k budget; 30k removed.
        // 单个 9 万字符结果获得全部 6 万预算，被截断 3 万字符。
        assert!(text.contains("[输出已截断：原输出 90000 字符，已截断 30000 字符]"));
        assert!(text.contains("已按比例截断"));
    });
}
