use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::Skill;
use super::config_io::path_key;
use super::frontmatter::parse_frontmatter;

/// Scan a search directory for skill subdirectories.
///
/// A direct child folder is a skill when it contains a `SKILL.md` file
/// (checked with `is_file`, so a directory named `SKILL.md` is not a
/// descriptor) whose frontmatter has a non-empty `description:` field.
/// The returned skills are sorted by folder path for deterministic output.
/// Stored paths are normalized with `std::path::absolute`, matching the
/// normalization `set_enabled` applies to its argument.
/// 扫描搜索目录中的技能子目录。
///
/// 直接子文件夹包含 `SKILL.md` 文件（以 `is_file` 校验，名为 `SKILL.md`
/// 的目录不算描述文件）且其 frontmatter 的 `description:` 非空时视为技能。
/// 返回的技能按目录路径排序以保证输出确定性。
pub(super) fn scan_skills_dir(
    search_dir: &Path,
    is_global: bool,
) -> crate::error::CoreResult<Vec<Skill>> {
    let mut skills = Vec::new();
    let read_dir = std::fs::read_dir(search_dir).map_err(|e| {
        crate::error::CoreError::Io(format!(
            "cannot read directory `{}`: {e}",
            search_dir.display()
        ))
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| {
            crate::error::CoreError::Io(format!(
                "cannot read entry in `{}`: {e}",
                search_dir.display()
            ))
        })?;
        let path = std::path::absolute(entry.path()).map_err(crate::error::CoreError::from)?;
        if !path.is_dir() {
            continue;
        }
        let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let descriptor = path.join("SKILL.md");
        if !descriptor.is_file() {
            continue;
        }
        let content =
            std::fs::read_to_string(&descriptor).map_err(crate::error::CoreError::from)?;
        let Some(frontmatter) = parse_frontmatter(&content) else {
            continue;
        };
        if frontmatter.description.trim().is_empty() {
            continue;
        }

        let name = frontmatter.name.unwrap_or_else(|| folder_name.to_string());
        skills.push(Skill {
            unique_name: name.clone(),
            name,
            description: frontmatter.description,
            body: frontmatter.body,
            path,
            is_global,
            is_enabled: !is_global,
        });
    }

    skills.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(skills)
}

/// Deduplicate skills and assign unique names.
///
/// Two skills are duplicates when `name`, `description` and `body` are all
/// equal; the kept copy is the project-scope one, otherwise the one with the
/// shorter absolute path. A colliding name is renamed as described in
/// [`Skill::unique_name`]. The result is sorted by unique name.
/// 去重技能并为每个技能分配唯一名称。
///
/// `name`、`description` 与 `body` 全部相同时视为重复；保留项目范围的
/// 副本，否则保留绝对路径更短者。与已注册名称冲突的技能按
/// [`Skill::unique_name`] 所述重命名。结果按唯一名称排序。
pub(super) fn dedup_skills(found: Vec<Skill>) -> Vec<Skill> {
    let mut kept: Vec<Skill> = Vec::new();
    for skill in found {
        if let Some(pos) = kept.iter().position(|kept| {
            kept.name == skill.name
                && kept.description == skill.description
                && kept.body == skill.body
        }) {
            if prefer_new(&kept[pos], &skill) {
                kept[pos] = skill;
            }
            continue;
        }
        kept.push(skill);
    }

    let mut used: HashSet<String> = HashSet::new();
    let mut result: Vec<Skill> = Vec::new();
    for mut skill in kept {
        let plain = skill.name.clone();
        if !used.contains(&plain) {
            skill.unique_name = plain;
        } else {
            let scope = if skill.is_global { "global" } else { "project" };
            let mut candidate = format!(".{scope}-{plain}");
            let mut counter = 2;
            while used.contains(&candidate) {
                candidate = format!(".{scope}-{plain}-{counter}");
                counter += 1;
            }
            skill.unique_name = candidate;
        }
        used.insert(skill.unique_name.clone());
        result.push(skill);
    }

    result.sort_by(|a, b| a.unique_name.cmp(&b.unique_name));
    result
}

/// Whether `candidate` should replace `existing` when both are exact
/// duplicates. Project scope beats global scope; within the same scope the
/// shorter absolute path string wins.
/// 精确重复时 `candidate` 是否应替换 `existing`。项目范围优先于全局范围；
/// 同一范围内绝对路径字符串更短者胜出。
fn prefer_new(existing: &Skill, candidate: &Skill) -> bool {
    match (existing.is_global, candidate.is_global) {
        (true, false) => true,
        (false, true) => false,
        _ => {
            let existing_len = existing.path.display().to_string().len();
            let candidate_len = candidate.path.display().to_string().len();
            candidate_len < existing_len
        }
    }
}

/// Apply enabled states from the config map to skills.
///
/// The default is `true` for project skills and `false` for global skills;
/// an entry keyed by the skill's `/`-separated path overrides the default.
/// 将配置映射中的启用状态应用到技能。
///
/// 默认项目技能为 `true`、全局技能为 `false`；以技能的 `/` 分隔路径为键的
/// 条目覆盖默认值。
pub(super) fn apply_enabled(skills: &mut [Skill], enabled: &HashMap<String, bool>) {
    for skill in skills {
        let default = !skill.is_global;
        skill.is_enabled = enabled
            .get(&path_key(&skill.path))
            .copied()
            .unwrap_or(default);
    }
}
