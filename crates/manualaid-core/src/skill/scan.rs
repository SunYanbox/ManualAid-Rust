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
/// Each skill records the `agent_dir` it was found under and receives the
/// stable unique name [`stable_unique_name`]. The returned skills are sorted
/// by folder path for deterministic output. Stored paths are normalized with
/// `std::path::absolute`, matching the normalization `set_enabled` applies
/// to its argument.
/// 扫描搜索目录中的技能子目录。
///
/// 直接子文件夹包含 `SKILL.md` 文件（以 `is_file` 校验，名为 `SKILL.md`
/// 的目录不算描述文件）且其 frontmatter 的 `description:` 非空时视为技能。
/// 每个技能记录其所属的 `agent_dir` 并获得稳定唯一名称
/// [`stable_unique_name`]。返回的技能按目录路径排序以保证输出确定性。
pub(super) fn scan_skills_dir(
    search_dir: &Path,
    agent_dir: &str,
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
            unique_name: stable_unique_name(is_global, agent_dir, &name),
            name,
            description: frontmatter.description,
            body: frontmatter.body,
            path,
            agent_dir: agent_dir.to_string(),
            is_global,
            is_enabled: !is_global,
        });
    }

    skills.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(skills)
}

/// Compose the stable unique name `<scope>-<agent_dir>-<name>`, the single
/// source of the naming format (`scope` is `project` or `global`).
/// 拼接稳定唯一名称 `<scope>-<agent_dir>-<name>`，是命名格式的唯一出处
/// （`scope` 为 `project` 或 `global`）。
pub(super) fn stable_unique_name(is_global: bool, agent_dir: &str, name: &str) -> String {
    let scope = if is_global { "global" } else { "project" };
    format!("{scope}-{agent_dir}-{name}")
}

/// Normalize text for duplicate comparison: `\r\n` line endings become `\n`
/// and surrounding whitespace is trimmed. Stored skill text is never
/// rewritten — normalization applies only while comparing.
/// 用于重复比较的文本归一化：`\r\n` 行尾转为 `\n` 并去除首尾空白。存储的
/// 技能文本不会被改写——归一化只发生在比较时。
fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

/// Whether two skills are exact duplicates: `name` equal, `description` and
/// `body` equal after normalization (see [`normalized`]).
/// 两个技能是否互为精确重复：`name` 相等，`description` 与 `body` 经
/// 归一化（见 [`normalized`]）后相等。
fn same_content(a: &Skill, b: &Skill) -> bool {
    a.name == b.name
        && normalized(&a.description) == normalized(&b.description)
        && normalized(&a.body) == normalized(&b.body)
}

/// Deduplicate skills and stabilize unique names.
///
/// Two skills are duplicates when their `name` is equal and their
/// `description` and `body` are equal after normalization; the kept copy is
/// the project-scope one, otherwise the one with the shorter absolute path.
/// Most skills keep the unique name assigned by the scanner. A residual
/// collision — two folders under the same scan root declaring the same
/// frontmatter `name` — is resolved deterministically: the first skill
/// (input order, which the scanner pre-sorts by path) keeps its name and
/// every later one gets the lowest `-N` suffix that is neither taken nor
/// another skill's natural name, so a real skill named e.g. `pdf-2` is never
/// shadowed. The result is sorted by unique name.
/// 去重技能并稳定唯一名称。
///
/// `name` 相等且 `description`、`body` 经归一化后相等时视为重复；保留
/// 项目范围的副本，否则保留绝对路径更短者。多数技能保留扫描器赋予的唯一
/// 名称。残余冲突——同一扫描
/// 根下两个目录声明相同 frontmatter `name`——确定性解决：首个技能（输入
/// 顺序，扫描器已按路径预排序）保留其名称，之后每个技能取最小的 `-N`
/// 后缀，且该后缀既未被占用也不是其他技能的自然名，这样名为 `pdf-2`
/// 的真实技能不会被遮蔽。结果按唯一名称排序。
pub(super) fn dedup_skills(found: Vec<Skill>) -> Vec<Skill> {
    let mut kept: Vec<Skill> = Vec::new();
    for skill in found {
        if let Some(pos) = kept.iter().position(|kept| same_content(kept, &skill)) {
            if prefer_new(&kept[pos], &skill) {
                kept[pos] = skill;
            }
            continue;
        }
        kept.push(skill);
    }

    let naturals: HashSet<String> = kept.iter().map(|s| s.unique_name.clone()).collect();
    let mut used: HashSet<String> = HashSet::new();
    let mut result: Vec<Skill> = Vec::new();
    for mut skill in kept {
        let natural = skill.unique_name.clone();
        let mut unique = natural.clone();
        let mut counter = 2;
        while used.contains(&unique) || (unique != natural && naturals.contains(&unique)) {
            unique = format!("{natural}-{counter}");
            counter += 1;
        }
        skill.unique_name = unique;
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
