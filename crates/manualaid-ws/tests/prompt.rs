//! Integration tests for system-prompt building and result formatting.
//! 系统提示词构建与结果格式化的集成测试。

use manualaid_core::parser::FormatRegistry;
use manualaid_core::skill::Skill;
use manualaid_core::tools::{ToolResult, UserActionKind};
use manualaid_ws::config::Config;
use manualaid_ws::prompt::{build_system_prompt, format_results, render_tools_list};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

/// `rust_i18n` keeps one process-wide locale, so every test that asserts
/// localized text must run under this lock to avoid races.
/// `rust_i18n` 的 locale 是进程级全局状态，所有断言本地化文本的测试必须
/// 在同一个锁下运行，避免并行竞态。
static LANG_LOCK: Mutex<()> = Mutex::new(());

const MAX: usize = 50_000;

/// Restores the process-wide locale to `en` on drop.
/// 在 drop 时将进程级 locale 恢复为 `en`。
struct LocaleRestore {
    _guard: MutexGuard<'static, ()>,
}

impl Drop for LocaleRestore {
    fn drop(&mut self) {
        i18n::set_locale("en");
    }
}

/// Run `f` with `lang` active, then restore English for other tests.
/// A poisoned lock is recovered so one failing test does not cascade.
/// 在 `lang` 语言下执行 `f`，结束后恢复英文供其他测试使用。
/// 锁被污染时直接接管，避免单个测试失败引发级联失败。
fn with_locale(lang: &str, f: impl FnOnce()) {
    let _restore = LocaleRestore {
        _guard: LANG_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    };
    i18n::set_locale(lang);
    f();
}

/// An isolated workspace root for tests that trigger truncation and thus
/// write persist files. Uses a unique per-test path under the system temp
/// directory so real workspaces are never touched.
/// 用于触发截断并写暂存文件的隔离工作区根目录。使用系统临时目录下的
/// 唯一路径，确保真实工作区永不被触碰。
fn test_workspace_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("manualaid-ws-prompt-{tag}-{}", std::process::id()))
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
fn system_prompt_renders_format_description_phrase() {
    // The format description is i18n text, not a literal key name.
    // 格式描述是 i18n 文案，而非字面键名。
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let prompt =
            build_system_prompt(&Config::default(), Path::new("C:/ws"), &registry, &[], &[]);
        assert!(!prompt.contains("cli.prompt.format_desc"));
        assert!(prompt.contains("The current tool-calling format is"));
    });
}

#[test]
fn system_prompt_renders_platform_notes_only_on_windows() {
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let prompt =
            build_system_prompt(&Config::default(), Path::new("C:/ws"), &registry, &[], &[]);
        if cfg!(windows) {
            assert!(prompt.contains("<platform-notes>"));
        } else {
            assert!(!prompt.contains("<platform-notes>"));
        }
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
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[], &[]);
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
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[], &[]);
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
            unique_name: "project-.claude-demo".to_string(),
            name: "demo".to_string(),
            description: "demo skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/demo"),
            agent_dir: ".claude".to_string(),
            is_global: false,
            is_enabled: true,
        };
        let prompt = build_system_prompt(&config, Path::new("C:/ws"), &registry, &[skill], &[]);
        assert!(prompt.contains("<skill-usage>"));
        assert!(prompt.contains("## skill"));
        assert!(prompt.contains("<available_skills>"));
        // The list shows the short exposed name, not the stable unique name.
        // 列表展示简短的暴露名，而非稳定唯一名称。
        assert!(prompt.contains("- demo: demo skill"));
        assert!(!prompt.contains("project-.claude-demo"));
    });
}

#[test]
fn system_prompt_skips_disabled_skills_in_list() {
    with_locale("en", || {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let enabled = Skill {
            unique_name: "project-.claude-demo".to_string(),
            name: "demo".to_string(),
            description: "demo skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/demo"),
            agent_dir: ".claude".to_string(),
            is_global: false,
            is_enabled: true,
        };
        let disabled = Skill {
            unique_name: "global-.claude-hidden".to_string(),
            name: "hidden".to_string(),
            description: "hidden skill".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/hidden"),
            agent_dir: ".claude".to_string(),
            is_global: true,
            is_enabled: false,
        };
        let prompt = build_system_prompt(
            &config,
            Path::new("C:/ws"),
            &registry,
            &[enabled, disabled],
            &[],
        );
        assert!(prompt.contains("<available_skills>"));
        assert!(prompt.contains("- demo: demo skill"));
        assert!(!prompt.contains("hidden"));
    });
}

#[test]
fn system_prompt_lists_dir_prefixed_names_for_collisions() {
    with_locale("en", || {
        let config = Config::default();
        let registry = FormatRegistry::new();
        let from_agents = Skill {
            unique_name: "project-.agents-word".to_string(),
            name: "word".to_string(),
            description: "agents copy".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/agents-word"),
            agent_dir: ".agents".to_string(),
            is_global: false,
            is_enabled: true,
        };
        let from_claude = Skill {
            unique_name: "project-.claude-word".to_string(),
            name: "word".to_string(),
            description: "claude copy".to_string(),
            body: "body".to_string(),
            path: PathBuf::from("/skills/claude-word"),
            agent_dir: ".claude".to_string(),
            is_global: false,
            is_enabled: true,
        };
        let prompt = build_system_prompt(
            &config,
            Path::new("C:/ws"),
            &registry,
            &[from_agents, from_claude],
            &[],
        );
        // The shared plain name is ambiguous, so the list shows the
        // dir-prefixed exposed names (issue example 3).
        // 共享裸名有歧义时列表展示目录前缀暴露名（issue 示例 3）。
        assert!(prompt.contains("- .agents-word: agents copy"));
        assert!(prompt.contains("- .claude-word: claude copy"));
        assert!(!prompt.contains("- word:"));
    });
}

#[test]
fn system_prompt_includes_selected_context_files() {
    with_locale("en", || {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-prompt-context-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "coverage >= 80%").unwrap();
        let config = Config::default();
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(
            &config,
            Path::new("C:/ws"),
            &registry,
            &[],
            &[root.join("AGENTS.md")],
        );
        assert!(prompt.contains(&i18n::t_str("prompt.system.context-files-reminder")));
        assert!(prompt.contains("Instructions from: AGENTS.md"));
        assert!(prompt.contains("coverage >= 80%"));
        assert!(prompt.ends_with("</system-reminder>"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn system_prompt_omits_context_when_auto_load_is_disabled() {
    with_locale("en", || {
        let root = std::env::temp_dir().join(format!(
            "manualaid-ws-prompt-context-off-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("AGENTS.md"), "secret rules").unwrap();
        let config = Config {
            context_auto_load: false,
            ..Config::default()
        };
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(
            &config,
            Path::new("C:/ws"),
            &registry,
            &[],
            &[root.join("AGENTS.md")],
        );
        // Assert on the reminder copy rather than the tag because the
        // top-of-prompt system-reminder note always contains the tag text.
        // 用引导语文案而非标签断言：提示词开头的 system-reminder 备注始终
        // 含有标签文本。
        assert!(!prompt.contains(&i18n::t_str("prompt.system.context-files-reminder")));
        assert!(!prompt.contains("Instructions from:"));
        assert!(!prompt.contains("secret rules"));
        assert!(prompt.ends_with("</system_prompt>"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn system_prompt_includes_git_status_snapshot_note() {
    // A real git repository is needed so `<git_information>` renders.
    // 需要真实 git 仓库才会输出 `<git_information>` 块。
    let root = std::env::temp_dir().join(format!("manualaid-ws-prompt-git-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .expect("failed to run git init");
    assert!(init.success(), "git init failed");
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&Config::default(), &root, &registry, &[], &[]);
        assert!(prompt.contains("<git_information>"));
        assert!(prompt.contains(
            "<git_information>\nThis is the git status at the start of the conversation."
        ));
        assert!(
            prompt.contains(
                "point-in-time snapshot and will not update during the conversation.\n\n"
            )
        );
    });
    with_locale("zh-CN", || {
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&Config::default(), &root, &registry, &[], &[]);
        assert!(prompt.contains("<git_information>"));
        assert!(prompt.contains(
            "<git_information>\n这是对话开始时的git状态。注意此状态是时间点快照，在对话期间不会更新。"
        ));
    });
    // Outside a git repository the block and the note are omitted.
    // 非 git 仓库时 git 块与备注均不输出。
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let prompt =
            build_system_prompt(&Config::default(), Path::new("C:/ws"), &registry, &[], &[]);
        assert!(!prompt.contains("<git_information>"));
        assert!(!prompt.contains("point-in-time snapshot"));
    });
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn system_prompt_includes_directory_listing_snapshot_note() {
    // A directory with at least one entry is needed so `<directory_listing>`
    // renders. Use a temp directory and write a dummy file to guarantee output.
    // 需要含至少一个条目的目录才会输出 `<directory_listing>` 块。
    // 使用临时目录并写入占位文件以确保非空。
    let root = std::env::temp_dir().join(format!("manualaid-ws-prompt-dl-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("placeholder.txt"), "").unwrap();
    with_locale("en", || {
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&Config::default(), &root, &registry, &[], &[]);
        assert!(prompt.contains("<directory_listing>"));
        assert!(prompt.contains(
            "<directory_listing>\nThis is the directory structure at the start of the conversation."
        ));
        assert!(
            prompt.contains(
                "point-in-time snapshot and will not update during the conversation.\n\n"
            )
        );
        // The closing tag must be on its own line, not glued to the last entry.
        // 闭合标签必须独占一行，不能与末尾条目粘连。
        assert!(
            prompt.contains("\n</directory_listing>\n"),
            "expected </directory_listing> on its own line"
        );
    });
    with_locale("zh-CN", || {
        let registry = FormatRegistry::new();
        let prompt = build_system_prompt(&Config::default(), &root, &registry, &[], &[]);
        assert!(prompt.contains("<directory_listing>"));
        assert!(prompt.contains(
            "<directory_listing>\n这是对话开始时的目录结构快照。注意此列表是时间点快照，在对话期间不会更新。"
        ));
    });
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn format_results_joins_multiple_results() {
    let results = vec![
        ToolResult::success("read", "a", true),
        ToolResult::failure("edit", "b"),
    ];
    let text = format_results(&results, MAX, Path::new(""));
    assert_eq!(text.matches("[TOOL_RESULT").count(), 2);
    assert!(text.contains("success=true"));
    assert!(text.contains("success=false"));
}

#[test]
fn format_results_embeds_summary_verbatim() {
    let result = ToolResult::success("read", "content", true)
        .with_params_summary("{\"file_path\":\"/a.txt\"}".into());
    let text = format_results(&[result], MAX, Path::new(""));
    assert!(text.contains("[TOOL_RESULT read success=true params={\"file_path\":\"/a.txt\"}]"));
    assert!(text.contains("success=true"));
}

#[test]
fn format_results_omits_empty_summary() {
    let result = ToolResult::success("shell", "done", false);
    let text = format_results(&[result], MAX, Path::new(""));
    assert!(text.contains("[TOOL_RESULT shell success=true]"));
    assert!(!text.contains("params="));
}

#[test]
fn format_results_empty_input_returns_empty_string() {
    assert_eq!(format_results(&[], MAX, Path::new("")), "");
}

#[test]
fn format_results_within_limit_is_unchanged() {
    let results = vec![
        ToolResult::success("read", "hello", true),
        ToolResult::failure("edit", "world"),
    ];
    let text = format_results(&results, MAX, Path::new(""));
    assert_eq!(
        text,
        "[TOOL_RESULT read success=true]\nhello\n[END TOOL_RESULT read]\n\n\
         [TOOL_RESULT edit success=false]\nworld\n[END TOOL_RESULT edit]"
    );
}

#[test]
fn format_results_preserves_whitespace_of_slices() {
    let result = ToolResult::success("read", "    indented\n  second  \n", true);
    let text = format_results(&[result], MAX, Path::new(""));
    assert!(text.contains("    indented\n  second  \n\n[END TOOL_RESULT read]"));
}

#[test]
fn format_results_truncates_proportionally_with_notices_and_warning() {
    with_locale("en", || {
        let root = test_workspace_root("trunc-prop");
        let results = vec![
            ToolResult::success("read", "χ".repeat(3_000), true),
            ToolResult::failure("shell", "λ".repeat(1_001)),
        ];
        let text = format_results(&results, 2_500, &root);
        // 3000 and 1001 out of 4001 total get 1500 and 1000 of the 2500
        // budget; the smaller result is pushed up to the floor and the
        // overshoot is cut from the larger one.
        // 3000 与 1001 字符按 4001 总量分配 2500 预算，分别得到 1500 与
        // 1000；较小结果被推到保底，超出部分从较大结果回扣。
        assert_eq!(text.matches('χ').count(), 1_500);
        assert_eq!(text.matches('λ').count(), 1_000);
        assert!(text.contains("[Output truncated: 1500 of 3000 chars removed]"));
        assert!(text.contains("[Output truncated: 1 of 1001 chars removed]"));
        assert!(text.contains("Output exceeded 2500 characters (total: 4001)"));
        assert!(text.contains("have been saved to"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_keeps_short_results_whole() {
    with_locale("en", || {
        let root = test_workspace_root("keep-short");
        let results = vec![
            ToolResult::success("read", "χ".repeat(3_000), true),
            ToolResult::failure("shell", "λ".repeat(50)),
        ];
        let text = format_results(&results, 1_500, &root);
        // The 50-char result is below the keep floor and stays untouched.
        // 50 字符的结果低于保底阈值，保持完整。
        assert_eq!(text.matches('λ').count(), 50);
        assert_eq!(text.matches('χ').count(), 1_450);
        assert_eq!(text.matches("[Output truncated:").count(), 1);
        assert!(text.contains("[Output truncated: 1550 of 3000 chars removed]"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_floor_overshoot_is_taken_from_largest_allocation() {
    with_locale("en", || {
        let root = test_workspace_root("floor-over");
        let results = vec![
            ToolResult::success("read", "χ".repeat(1_001), true),
            ToolResult::failure("shell", "λ".repeat(10_000)),
        ];
        let text = format_results(&results, 1_050, &root);
        // Both results sit at the 1000-char floor because the budget cannot
        // cover them; the overshoot stays unpaid.
        // 预算无法覆盖两个结果，二者都停在 1000 字符保底；超出部分无法回扣。
        assert_eq!(text.matches('χ').count(), 1_000);
        assert_eq!(text.matches('λ').count(), 1_000);
        assert!(text.contains("[Output truncated: 1 of 1001 chars removed]"));
        assert!(text.contains("[Output truncated: 9000 of 10000 chars removed]"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_all_short_drops_whole_results_from_the_end() {
    with_locale("en", || {
        let root = test_workspace_root("drop-end");
        let results = vec![
            ToolResult::success("read", "χ".repeat(500), true),
            ToolResult::failure("edit", "λ".repeat(500)),
            ToolResult::success("shell", "π".repeat(500), false),
        ];
        let text = format_results(&results, 1_000, &root);
        // Nothing can be shortened, so whole results are dropped from the end.
        // 没有可缩短的结果，从末尾整块丢弃。
        assert_eq!(text.matches("[TOOL_RESULT").count(), 2);
        assert_eq!(text.matches('χ').count(), 500);
        assert_eq!(text.matches('λ').count(), 500);
        assert_eq!(text.matches('π').count(), 0);
        assert!(text.contains("Output exceeded 1000 characters (total: 1500)"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_unpayable_overshoot_stays_at_floor() {
    with_locale("en", || {
        let root = test_workspace_root("unpayable");
        let results = vec![
            ToolResult::success("read", "χ".repeat(1_001), true),
            ToolResult::failure("edit", "λ".repeat(1_001)),
            ToolResult::success("shell", "π".repeat(1_001), false),
        ];
        let text = format_results(&results, 1_500, &root);
        // Every result's proportional share falls below the 1000-char floor,
        // so all three sit at the floor and the overshoot stays unpaid.
        // 每个结果的按比例份额都低于 1000 字符保底，三者都停在保底值，
        // 超出预算的部分无法回扣。
        assert_eq!(text.matches('χ').count(), 1_000);
        assert_eq!(text.matches('λ').count(), 1_000);
        assert_eq!(text.matches('π').count(), 1_000);
        assert_eq!(text.matches("[Output truncated:").count(), 3);
        assert!(text.contains("Output exceeded 1500 characters (total: 3003)"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_notice_is_localized() {
    with_locale("zh-CN", || {
        let root = test_workspace_root("localized");
        let results = vec![ToolResult::success("read", "χ".repeat(3_000), true)];
        let text = format_results(&results, 2_000, &root);
        // The single 3000-char result gets the whole 2000 budget; 1000 removed.
        // 单个 3000 字符结果获得全部 2000 预算，被截断 1000 字符。
        assert!(text.contains("[输出已截断：原输出 3000 字符，已截断 1000 字符]"));
        assert!(text.contains("已按比例截断"));
        assert!(text.contains("暂存"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_persists_full_output_when_truncated() {
    with_locale("en", || {
        let root = test_workspace_root("persist-full");
        let results = vec![
            ToolResult::success("read", "χ".repeat(900), true)
                .with_params_summary("file_path=\"/a.txt\"".to_string()),
            ToolResult::failure("shell", "λ".repeat(300)),
        ];
        let text = format_results(&results, 600, &root);
        assert!(text.contains("have been saved to"));
        assert!(text.contains("- read (file_path=\"/a.txt\"): line 1"));
        assert!(text.contains("- shell: line"));

        // The persisted file must exist under `.ManualAid/temp/` and contain
        // the complete un-truncated tool outputs.
        // 暂存文件必须存在于 `.ManualAid/temp/` 下，并包含完整的未截断工具输出。
        let temp_dir = root.join(".ManualAid").join("temp");
        assert!(temp_dir.is_dir());
        let saved: Vec<_> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(saved.len(), 1);
        let content = std::fs::read_to_string(saved[0].path()).unwrap();
        assert_eq!(content.matches('χ').count(), 900);
        assert_eq!(content.matches('λ').count(), 300);
        assert!(content.contains("[TOOL_RESULT read success=true params=file_path=\"/a.txt\"]"));
        assert!(content.contains("[END TOOL_RESULT read]"));
        assert!(content.contains("[TOOL_RESULT shell success=false]"));
        assert!(content.contains("[END TOOL_RESULT shell]"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_no_file_when_within_limit() {
    with_locale("en", || {
        let root = test_workspace_root("within-limit");
        let results = vec![ToolResult::success("read", "hello", true)];
        let text = format_results(&results, 500, &root);
        assert!(!text.contains("saved to"));
        assert!(!root.join(".ManualAid").exists());
    });
}

#[test]
fn format_results_persist_failure_silently_degrades() {
    with_locale("en", || {
        // A workspace root pointing at a regular file makes `create_dir_all`
        // fail, so the persisted notice must be absent while the original
        // truncation warning stays.
        // 将工作区根指向一个普通文件会使 `create_dir_all` 失败，因此暂存
        // 通知必须缺失，而原有截断警告保留。
        let root = test_workspace_root("fail-path");
        std::fs::create_dir_all(&root).unwrap();
        let file_path = root.join("blocker");
        std::fs::write(&file_path, "x").unwrap();
        let results = vec![ToolResult::success("read", "χ".repeat(3_000), true)];
        let text = format_results(&results, 2_000, &file_path);
        assert!(text.contains("Output exceeded"));
        assert!(!text.contains("have been saved to"));
        let _ = std::fs::remove_dir_all(&root);
    });
}

#[test]
fn format_results_wraps_user_action_verbatim_within_limit() {
    let result = ToolResult::success("shell", "out", false)
        .with_user_action(UserActionKind::Exec, "cargo test");
    let text = format_results(&[result], MAX, Path::new(""));
    assert_eq!(
        text,
        "[USER_ACTION kind=\"exec\" command=\"cargo test\"]\nout\n[END USER_ACTION]"
    );
}

#[test]
fn format_results_mixes_user_action_and_tool_result() {
    let exec = ToolResult::success("shell", "exec out", false)
        .with_user_action(UserActionKind::Exec, "cargo test");
    let regular = ToolResult::success("read", "read out", true);
    let text = format_results(&[exec, regular], MAX, Path::new(""));
    assert!(text.contains("[USER_ACTION kind=\"exec\" command=\"cargo test\"]"));
    assert!(text.contains("[END USER_ACTION]"));
    assert!(text.contains("[TOOL_RESULT read success=true]"));
    assert!(text.contains("[END TOOL_RESULT read]"));
}

#[test]
fn format_results_truncates_user_action_with_same_pipeline() {
    with_locale("en", || {
        let root = test_workspace_root("user-action-trunc");
        let result = ToolResult::success("shell", "χ".repeat(3_000), false)
            .with_user_action(UserActionKind::Exec, "cargo test");
        let text = format_results(&[result], 2_000, &root);
        assert_eq!(text.matches('χ').count(), 2_000);
        assert!(text.contains("[USER_ACTION kind=\"exec\" command=\"cargo test\"]"));
        assert!(text.contains("[END USER_ACTION]"));
        assert!(text.contains("[Output truncated: 1000 of 3000 chars removed]"));
        // The persisted list uses `kind (label)` for user actions.
        assert!(text.contains("- exec (cargo test): line 1"));

        // The persisted file must contain the complete user action wrapper.
        let temp_dir = root.join(".ManualAid").join("temp");
        let saved: Vec<_> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(saved.len(), 1);
        let content = std::fs::read_to_string(saved[0].path()).unwrap();
        assert_eq!(content.matches('χ').count(), 3_000);
        assert!(content.contains("[USER_ACTION kind=\"exec\" command=\"cargo test\"]"));
        assert!(content.contains("[END USER_ACTION]"));
        let _ = std::fs::remove_dir_all(&root);
    });
}
