//! Candidate providers for the completion UI: built-in `/commands` and
//! enabled skills.
//! 补全 UI 的候选提供者：内置 `/命令` 与已启用技能。

use manualaid_core::skill;

use crate::commands::loop_cli::inline::BUILTIN_COMMANDS;

/// One suggestion row shown by the completion UI.
/// 补全 UI 显示的一条建议。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    /// Text filled into the input line when the suggestion is accepted
    /// (e.g. `/help`, `project-.claude-demo`).
    /// 选中建议后填入输入行的文本（如 `/help`、`project-.claude-demo`）。
    pub label: String,
    /// Short usage hint rendered next to the label.
    /// 显示在候选旁的简短用途说明。
    pub description: String,
    /// Whether the suggestion is a directory; path completion uses this to
    /// append `/` after Tab so the user can keep browsing inside it.
    /// 建议是否为目录；路径补全据此在 Tab 后补 `/`，方便继续浏览其内部。
    pub is_dir: bool,
    /// Whether the suggestion is ignored by the project's ignore rules;
    /// ignored entries are still shown but rendered dimmed.
    /// 建议是否被项目忽略规则忽略；被忽略条目仍显示，但以弱化样式渲染。
    pub dimmed: bool,
}

/// Build candidates for every built-in inline command, localized through
/// the shared `cli.cmd.*` description keys.
/// 为所有内置内联命令构建候选，描述经共享的 `cli.cmd.*` 键本地化。
pub(crate) fn builtin_commands() -> Vec<Candidate> {
    BUILTIN_COMMANDS
        .iter()
        .map(|command| Candidate {
            label: command.name.to_owned(),
            description: i18n::t_str(command.desc_key),
            is_dir: false,
            dimmed: false,
        })
        .collect()
}

/// Build candidates for the currently enabled skills; the label is the
/// stable unique name that the skill tool accepts.
/// 为当前已启用的技能构建候选；候选文本是 skill 工具接受的稳定唯一名。
pub(crate) fn skills() -> Vec<Candidate> {
    skill::enabled_skills()
        .into_iter()
        .map(|item| Candidate {
            label: item.unique_name,
            description: item.description,
            is_dir: false,
            dimmed: false,
        })
        .collect()
}

/// Keep the candidates whose label starts with `query`, case-insensitively;
/// an empty query keeps everything.
/// 保留候选文本以 `query` 开头（大小写不敏感）的候选；空查询保留全部。
pub(crate) fn filter(candidates: Vec<Candidate>, query: &str) -> Vec<Candidate> {
    let needle = query.to_lowercase();
    candidates
        .into_iter()
        .filter(|candidate| candidate.label.to_lowercase().starts_with(&needle))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::{LOCALE_LOCK, SKILL_LOCK, temp_dir};

    #[test]
    fn builtin_commands_come_from_the_shared_constant() {
        let _lock = LOCALE_LOCK.lock().unwrap();
        i18n::set_locale("en");
        let candidates = builtin_commands();
        assert!(candidates.len() >= 10);
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.label.starts_with('/'))
        );
        let help = candidates.iter().find(|c| c.label == "/help").unwrap();
        assert_eq!(help.description, "Show this help");
        let copy = candidates.iter().find(|c| c.label == "/c").unwrap();
        assert_eq!(copy.description, "Copy round result");
    }

    #[test]
    fn filter_keeps_case_insensitive_prefix_matches() {
        let candidates = vec![
            Candidate {
                label: "/help".to_owned(),
                description: "Show this help".to_owned(),
                is_dir: false,
                dimmed: false,
            },
            Candidate {
                label: "/history".to_owned(),
                description: "Show tool history".to_owned(),
                is_dir: false,
                dimmed: false,
            },
            Candidate {
                label: "project-.claude-demo".to_owned(),
                description: "demo".to_owned(),
                is_dir: false,
                dimmed: false,
            },
        ];
        assert_eq!(filter(candidates.clone(), "").len(), 3);
        // The query keeps the leading `/` exactly as typed at the prompt.
        // 查询保留在提示符处输入的起始 `/`。
        let hits = filter(candidates, "/HI");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label, "/history");
    }

    #[test]
    fn skills_list_only_the_enabled_ones() {
        let _lock = SKILL_LOCK.lock().unwrap();
        let root = temp_dir("complete_skills");
        let home = root.join("home");
        let project_skill = root.join(".claude").join("skills").join("demo");
        fs::create_dir_all(&project_skill).unwrap();
        fs::write(
            project_skill.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill for completion\n---\nbody\n",
        )
        .unwrap();
        let global_skill = home.join(".claude").join("skills").join("extra");
        fs::create_dir_all(&global_skill).unwrap();
        fs::write(
            global_skill.join("SKILL.md"),
            "---\nname: extra\ndescription: Disabled global skill\n---\nbody\n",
        )
        .unwrap();

        skill::reload_skills_with_home(&root, &home).unwrap();
        let candidates = skills();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "project-.claude-demo");
        assert_eq!(candidates[0].description, "Demo skill for completion");

        // Leave the shared skill store empty for the other tests.
        // 为其他测试把共享技能存储清空。
        let empty = temp_dir("complete_skills_empty");
        skill::reload_skills_with_home(&empty, &empty).unwrap();
    }
}
