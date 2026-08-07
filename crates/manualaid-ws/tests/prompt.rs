//! Integration tests for system-prompt building and result formatting.
//! 系统提示词构建与结果格式化的集成测试。

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::Skill;
use manualaid_core::tools::ToolResult;
use manualaid_ws::config::Config;
use manualaid_ws::prompt::{build_system_prompt, format_results, render_tools_list};
use std::path::{Path, PathBuf};

#[test]
fn tools_list_uses_localized_descriptions() {
    i18n::set_locale("en");
    let registry = FormatRegistry::new();
    let list = render_tools_list(&Config::default(), &registry);
    assert!(list.contains("## read"));
    assert!(list.contains("absolute path"));
    i18n::set_locale("zh-CN");
    let list = render_tools_list(&Config::default(), &registry);
    assert!(list.contains("绝对路径"));
    i18n::set_locale("en");
}

#[test]
fn system_prompt_reflects_config_switches() {
    i18n::set_locale("en");
    let config = Config {
        skill: false,
        ..Config::default()
    };
    let registry = FormatRegistry::new();
    let prompt = build_system_prompt(&config, std::path::Path::new("C:/ws"), &registry, &[]);
    assert!(prompt.contains("<system_prompt>"));
    assert!(prompt.contains("C:/ws"));
    assert!(!prompt.contains("<skill-usage>"));
    i18n::set_locale("en");
}

#[test]
fn system_prompt_drops_skill_when_none_are_enabled() {
    i18n::set_locale("en");
    let config = Config::default();
    let registry = FormatRegistry::new();
    let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[]);
    assert!(!prompt.contains("<skill-usage>"));
    assert!(!prompt.contains("## skill"));
    assert!(!prompt.contains("<available_skills>"));
    i18n::set_locale("en");
}

#[test]
fn system_prompt_includes_enabled_skills() {
    i18n::set_locale("en");
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
    i18n::set_locale("en");
}

#[test]
fn format_results_joins_multiple_results() {
    let results = vec![
        ToolResult::success("read", "a", true),
        ToolResult::failure("edit", "b"),
    ];
    let text = format_results(&results);
    assert_eq!(text.matches("<tool_result").count(), 2);
    assert!(text.contains("success=\"true\""));
    assert!(text.contains("success=\"false\""));
}
