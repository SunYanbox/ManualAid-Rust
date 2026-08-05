//! Discovery, deduplication and enable/disable management for ManualAid
//! skills.
//! ManualAid 技能的发现、去重与启用/禁用管理。
//!
//! # Description
//! For each agent directory in `AGENT_DIRS`, skills are searched in
//! `<project_root>/<agent_dir>/skills` (project scope) and
//! `<home>/<agent_dir>/skills` (global scope). A directory `D/skills/<name>`
//! is a skill when it directly contains a `SKILL.md` file whose YAML
//! frontmatter has a non-empty `description:` field.
//!
//! **Windows note:** hand-editing the config file requires `/` as the path
//! separator and quoted keys (e.g. `"C:/Users/..." = true`); `\` and
//! unquoted keys do not match. Entries written by [`set_enabled`] always use
//! the `/` form.
//! # Test notes
//! The error branches of `read_dir` / entry iteration in the scanner and
//! the skip for non-UTF-8 folder names depend on OS state that tests cannot
//! construct portably; these branches are not required to have high test
//! coverage.
//! # 描述
//! 对 `AGENT_DIRS` 中每个 agent 目录，在 `<项目根>/<agent_dir>/skills`
//! （项目范围）与 `<home>/<agent_dir>/skills`（全局范围）中搜索技能；目录
//! `D/skills/<name>` 直接包含 `SKILL.md` 文件且其 YAML frontmatter 含非空
//! `description:` 字段时即视为技能。
//!
//! **Windows 注意：** 手写 config 时必须使用 `/` 作为路径分隔符且键需加
//! 引号（如 `"C:/Users/..." = true`）；`\` 与未加引号的键无法匹配。
//! [`set_enabled`] 写入的条目始终使用 `/` 形式。
//! # 测试说明
//! 扫描器中 `read_dir` / 条目遍历的错误分支以及非 UTF-8 文件夹名的跳过
//! 分支依赖测试无法跨平台构造的 OS 状态；这些分支不要求高测试覆盖率。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};
use toml::Table;
use toml_edit::{DocumentMut, Item, Table as TomlEditTable, Value as TomlEditValue};

use crate::error::{CoreError, CoreResult};
use crate::file_io;
use crate::manualaid_dir::DEFAULT_PROJECT_CONFIG_CONTENT;
use crate::user_dir;

/// Agent configuration directories scanned for `skills/` subdirectories.
/// Project scope is scanned before the global scope, so identical skills
/// keep the project copy.
/// 被扫描 `skills/` 子目录的 agent 配置目录列表。
/// 项目范围先于全局范围扫描，确保重复技能保留项目副本。
const AGENT_DIRS: &[&str] = &[
    ".claude",
    ".agent",
    ".codex",
    ".cc-switch",
    ".agents",
    ".iflow",
    ".ManualAid",
    ".opencode",
];

/// A discovered ManualAid skill.
/// 已发现的 ManualAid 技能。
///
/// # Description
/// Skills are loaded by [`reload_skills`] from the project and global search
/// directories described in the module documentation. `unique_name` is the
/// deduplicated lookup key used by [`get_skill`]: it equals `name` unless a
/// name collision forces a `.{scope}-{name}` renaming (`scope` is `project`
/// or `global`).
/// # 描述
/// 技能由 [`reload_skills`] 从模块文档所述的项目与全局搜索目录加载。
/// `unique_name` 是 [`get_skill`] 使用的去重后的查找键：除非名称冲突导致
/// `.{scope}-{name}` 重命名（`scope` 为 `project` 或 `global`），否则与
/// `name` 相同。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// Deduplicated lookup key, equal to `name` when there is no collision.
    /// 去重后的查找键；无冲突时与 `name` 相同。
    pub unique_name: String,
    /// Display name: the frontmatter `name:` field, falling back to the
    /// skill folder name when the field is empty or missing.
    /// 显示名称：frontmatter 的 `name:` 字段；字段为空或缺失时回退为
    /// 技能目录名。
    pub name: String,
    /// The frontmatter `description:` field (never empty for loaded skills).
    /// frontmatter 的 `description:` 字段（已加载技能永不为空）。
    pub description: String,
    /// Markdown body after the closing frontmatter delimiter.
    /// 结束 frontmatter 分隔符之后的 Markdown 正文。
    pub body: String,
    /// Absolute path of the skill folder, normalized with `std::path::absolute`.
    /// 技能目录的绝对路径。
    pub path: PathBuf,
    /// `true` for skills under `<home>/<agent_dir>/skills`.
    /// 位于 `<home>/<agent_dir>/skills` 下的技能为 `true`。
    pub is_global: bool,
    /// Whether the skill is enabled; defaults to `true` for project skills
    /// and `false` for global skills.
    /// 技能是否启用；默认项目技能为 `true`，全局技能为 `false`。
    pub is_enabled: bool,
}

/// The in-memory skill store, guarded by a single `RwLock` so readers never
/// observe a mix of old and new state.
/// 内存中的技能存储；由单一 `RwLock` 保护，读取方不会观察到新旧状态的混合。
struct SkillStore {
    skills: Vec<Skill>,
    project_root: Option<PathBuf>,
}

static STORE: RwLock<SkillStore> = RwLock::new(SkillStore {
    skills: Vec::new(),
    project_root: None,
});

/// Acquire the store read guard, recovering from a poisoned lock with a
/// one-time warning. Recovery is safe because `reload_skills` rebuilds memory
/// from disk.
fn read_store() -> RwLockReadGuard<'static, SkillStore> {
    STORE.read().unwrap_or_else(|poisoned| {
        file_io::warn_poisoned_lock("STORE");
        poisoned.into_inner()
    })
}

/// Acquire the store write guard, recovering from a poisoned lock with a
/// one-time warning. Recovery is safe because the config file remains the
/// durable source of truth.
fn write_store() -> RwLockWriteGuard<'static, SkillStore> {
    STORE.write().unwrap_or_else(|poisoned| {
        file_io::warn_poisoned_lock("STORE");
        poisoned.into_inner()
    })
}

/// Rescan all skill search directories (see the module documentation) and
/// replace the in-memory skill store in one write.
/// 重新扫描所有技能搜索目录（见模块文档），并一次性替换内存中的技能存储。
///
/// # Description
/// Missing directories are treated as empty and never raise an error.
/// The normalized project root is remembered so [`set_enabled`] can
/// persist changes; on error the previous store is left untouched.
/// # 描述
/// 缺失的目录视为空且不报错。规范化的项目根会被保存，以便 [`set_enabled`]
/// 持久化变更；出错时原有存储保持不变。
pub fn reload_skills(project_root: &Path) -> CoreResult<()> {
    let home = user_dir::home_dir()?;
    reload_skills_impl(project_root, &home)
}

/// Like [`reload_skills`] but with an explicit home directory instead of the
/// real user home, used by tests to exercise the global scope without
/// touching the real home. Hidden from docs.
/// 同 [`reload_skills`]，但以显式指定的主目录代替真实用户主目录，供测试
/// 在不触碰真实主目录的情况下覆盖全局范围。文档中隐藏。
#[doc(hidden)]
pub fn reload_skills_with_home(project_root: &Path, home: &Path) -> CoreResult<()> {
    reload_skills_impl(project_root, home)
}

/// Return an immutable snapshot of all loaded skills.
/// 返回所有已加载技能的不可变快照。
pub fn all_skills() -> Vec<Skill> {
    read_store().skills.clone()
}

/// Return an immutable snapshot of all enabled skills.
/// 返回所有已启用技能的不可变快照。
pub fn enabled_skills() -> Vec<Skill> {
    all_skills()
        .into_iter()
        .filter(|skill| skill.is_enabled)
        .collect()
}

/// Return the loaded skill whose unique name matches, or `None`.
/// 返回唯一名称匹配的已加载技能，未找到时返回 `None`。
pub fn get_skill(unique_name: &str) -> Option<Skill> {
    read_store()
        .skills
        .iter()
        .find(|skill| skill.unique_name == unique_name)
        .cloned()
}

/// Persist the enabled state of the skill at `path` to
/// `<project_root>/.ManualAid/config.toml` (created together with its parent
/// directories when missing) and update the in-memory store.
/// 将 `path` 处技能的启用状态持久化到 `<项目根>/.ManualAid/config.toml`
/// （缺失时连同父目录一起创建），并更新内存存储。
///
/// # Description
/// Returns `CoreError::NotFound` when [`reload_skills`] has not been called
/// or `path` does not match a loaded skill; in the latter case nothing is
/// persisted. `path` must refer to the stored skill folder path (see
/// [`Skill::path`]). Existing entries for other skills and hand-written
/// sections are preserved; persistence happens before the in-memory update.
/// The whole check-persist-update sequence runs under the store write lock,
/// so concurrent calls cannot lose updates and a concurrent [`reload_skills`]
/// cannot interleave between the config write and the in-memory update.
/// # 描述
/// 当 [`reload_skills`] 尚未调用、或 `path` 不匹配任何已加载技能时返回
/// `CoreError::NotFound`，后者不会写入任何内容。`path` 必须指向存储的
/// 技能目录路径（参见 [`Skill::path`]）。配置中的其他条目与用户手写
/// 配置节会被保留；先持久化后更新内存。
pub fn set_enabled(path: &Path, enabled: bool) -> CoreResult<()> {
    let path = std::path::absolute(path).map_err(CoreError::from)?;

    // Hold the write lock for the whole check-persist-update sequence: it
    // makes concurrent `set_enabled` calls atomic with respect to the store
    // and prevents `reload_skills` from interleaving between the config
    // write and the in-memory update. File I/O under the lock is acceptable
    // for this low-frequency management operation.
    let mut store = write_store();

    let root = store.project_root.clone().ok_or_else(|| {
        CoreError::NotFound("reload_skills must be called before set_enabled".to_string())
    })?;
    if !store.skills.iter().any(|skill| skill.path == path) {
        return Err(CoreError::NotFound(format!(
            "no loaded skill matches path `{}`",
            path.display()
        )));
    }

    let config_path = root.join(".ManualAid").join("config.toml");
    let mut enabled_map = read_enabled_map(&config_path)?;
    enabled_map.insert(path_key(&path), enabled);
    write_enabled_map(&config_path, &enabled_map)?;

    if let Some(skill) = store.skills.iter_mut().find(|skill| skill.path == path) {
        skill.is_enabled = enabled;
    }
    Ok(())
}

/// Clear the in-memory skill store and the remembered project root.
/// Hidden from docs because it exists for tests to restore state.
/// 清空内存中的技能存储与已保存的项目根。文档中隐藏，因为它供测试恢复
/// 状态用。
#[doc(hidden)]
pub fn reset_skills() {
    let mut store = write_store();
    store.skills.clear();
    store.project_root = None;
}

fn reload_skills_impl(project_root: &Path, home: &Path) -> CoreResult<()> {
    let root = std::path::absolute(project_root).map_err(CoreError::from)?;
    // Normalize the home directory too, so global skill paths are stored in
    // the same form `set_enabled` produces for its lookup.
    let home = std::path::absolute(home).map_err(CoreError::from)?;

    let mut found = Vec::new();
    for agent_dir in AGENT_DIRS {
        let search_dir = root.join(agent_dir).join("skills");
        if search_dir.is_dir() {
            found.extend(scan_skills_dir(&search_dir, false)?);
        }
    }
    for agent_dir in AGENT_DIRS {
        let search_dir = home.join(agent_dir).join("skills");
        if search_dir.is_dir() {
            found.extend(scan_skills_dir(&search_dir, true)?);
        }
    }

    let mut deduped = dedup_skills(found);
    let config_path = root.join(".ManualAid").join("config.toml");
    let enabled = read_enabled_map(&config_path)?;
    apply_enabled(&mut deduped, &enabled);

    let mut store = write_store();
    store.skills = deduped;
    store.project_root = Some(root);
    Ok(())
}

/// Frontmatter fields extracted from a `SKILL.md` file.
/// 从 `SKILL.md` 文件提取的 frontmatter 字段。
struct Frontmatter {
    /// `name:` value when present and non-empty; the caller falls back to
    /// the folder name when missing.
    /// `name:` 存在且非空时的值；缺失时由调用方回退为目录名。
    name: Option<String>,
    /// `description:` value; may be empty, the caller decides to skip.
    /// `description:` 的值；可能为空，由调用方决定跳过。
    description: String,
    /// Markdown after the closing `---` delimiter.
    /// 结束 `---` 分隔符之后的 Markdown 正文。
    body: String,
}

/// Parse YAML-style frontmatter and body from a `SKILL.md` content.
///
/// Returns `Some` whenever a leading `---` block exists, with `name` as
/// `None` when the field is missing or empty; `description` may be empty.
/// Returns `None` when no leading `---` block exists.
/// 从 `SKILL.md` 内容解析 YAML 风格 frontmatter 与正文。
///
/// 只要存在前导 `---` 块即返回 `Some`，`name` 字段缺失或为空时为 `None`；
/// `description` 可能为空。无前导 `---` 块时返回 `None`。
fn parse_frontmatter(content: &str) -> Option<Frontmatter> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    let after_opener = &trimmed[3..];
    let end = after_opener.find("\n---")?;
    let frontmatter = after_opener[..end].trim();

    let after_closing = &after_opener[end + 4..];
    let body = match after_closing.find('\n') {
        Some(nl) => after_closing[nl + 1..].to_string(),
        None => String::new(),
    };

    let mut name = None;
    let mut description = None;

    #[derive(Clone, Copy)]
    enum BlockMode {
        /// YAML `>` folded block — newlines fold to spaces, blank lines
        /// become paragraph breaks.
        /// YAML `>` 折叠块——换行折叠为空格，空行变为段落分隔。
        Folded,
        /// YAML `|` literal block — newlines preserved.
        /// YAML `|` 字面量块——保留换行符。
        Literal,
    }

    let mut collect: Option<(String, BlockMode)> = None;
    let mut blank_line = false;

    for raw_line in frontmatter.lines() {
        if let Some((ref mut acc, mode)) = collect {
            if raw_line.starts_with(' ') || raw_line.is_empty() {
                let content = raw_line.trim();
                if content.is_empty() {
                    match mode {
                        BlockMode::Folded => blank_line = true,
                        BlockMode::Literal => acc.push('\n'),
                    }
                    continue;
                }
                if blank_line {
                    acc.push('\n');
                    blank_line = false;
                } else if !acc.is_empty() {
                    match mode {
                        BlockMode::Folded => acc.push(' '),
                        BlockMode::Literal => acc.push('\n'),
                    }
                }
                acc.push_str(content);
                continue;
            }

            description = Some(std::mem::take(acc));
            collect = None;
        }

        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key == "name" && !value.is_empty() {
                name = Some(value.to_string());
            }
            if key == "description" {
                if value == ">" || value == ">-" || value == ">+" {
                    collect = Some((String::new(), BlockMode::Folded));
                } else if value == "|" || value == "|-" || value == "|+" {
                    collect = Some((String::new(), BlockMode::Literal));
                } else if value.is_empty() {
                    collect = Some((String::new(), BlockMode::Folded));
                } else {
                    description = Some(value.to_string());
                }
            }
        }
    }

    if let Some((acc, _)) = collect
        && !acc.is_empty()
        && description.is_none()
    {
        description = Some(acc);
    }

    Some(Frontmatter {
        name,
        description: description.unwrap_or_default(),
        body,
    })
}

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
fn scan_skills_dir(search_dir: &Path, is_global: bool) -> CoreResult<Vec<Skill>> {
    let mut skills = Vec::new();
    let read_dir = std::fs::read_dir(search_dir).map_err(|e| {
        CoreError::Io(format!(
            "cannot read directory `{}`: {e}",
            search_dir.display()
        ))
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| {
            CoreError::Io(format!(
                "cannot read entry in `{}`: {e}",
                search_dir.display()
            ))
        })?;
        let path = std::path::absolute(entry.path()).map_err(CoreError::from)?;
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
        let content = std::fs::read_to_string(&descriptor).map_err(CoreError::from)?;
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
fn dedup_skills(found: Vec<Skill>) -> Vec<Skill> {
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
fn apply_enabled(skills: &mut [Skill], enabled: &HashMap<String, bool>) {
    for skill in skills {
        let default = !skill.is_global;
        skill.is_enabled = enabled
            .get(&path_key(&skill.path))
            .copied()
            .unwrap_or(default);
    }
}

/// The config-file key for a skill path: the absolute path with `\`
/// replaced by `/` so keys are identical across platforms.
/// 技能路径的配置键：绝对路径的 `\` 替换为 `/`，使键跨平台一致。
fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Read the `[skill]` table of a config file into a path-keyed map.
///
/// A missing file yields an empty map. TOML syntax errors and non-boolean
/// values are `CoreError::Config` errors.
/// 读取配置文件中的 `[skill]` 表为路径键映射。
///
/// 文件缺失时返回空映射。TOML 语法错误与非布尔值返回
/// `CoreError::Config` 错误。
fn read_enabled_map(config_path: &Path) -> CoreResult<HashMap<String, bool>> {
    file_io::with_file_lock(config_path, || {
        let content = match std::fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "cannot read skill config `{}`: {e}",
                    config_path.display()
                )));
            }
        };
        let table: Table = toml::from_str(&content)?;
        let mut enabled = HashMap::new();
        if let Some(toml::Value::Table(skill)) = table.get("skill") {
            for (key, value) in skill {
                match value {
                    toml::Value::Boolean(value) => {
                        enabled.insert(key.clone(), *value);
                    }
                    _ => {
                        return Err(CoreError::Config(format!(
                            "skill config entry `{key}` must be a boolean"
                        )));
                    }
                }
            }
        }
        Ok(enabled)
    })
}

/// Write a path-keyed map back into the `[skill]` table of a config file,
/// creating parent directories and the file itself when missing. Other
/// tables, comments and formatting in an existing file are preserved; a
/// missing or blank file is seeded from the default project template.
/// 将路径键映射写回配置文件的 `[skill]` 表，缺失时创建父目录与文件本身。
/// 已有文件中的其他配置节、注释与格式会被保留；文件缺失或空白时以
/// 默认项目模板为基底创建。
fn write_enabled_map(config_path: &Path, enabled: &HashMap<String, bool>) -> CoreResult<()> {
    file_io::with_file_lock(config_path, || {
        let content = match std::fs::read_to_string(config_path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "cannot read skill config `{}`: {e}",
                    config_path.display()
                )));
            }
        };

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::Io(format!(
                    "cannot create skill config directory `{}`: {e}",
                    parent.display()
                ))
            })?;
        }

        // Missing or blank files are seeded from the default project
        // template so the generated config always carries its comments.
        // 缺失或空白的配置文件以默认项目模板为基底，保证文件始终带说明注释。
        let mut doc = if content.trim().is_empty() {
            DEFAULT_PROJECT_CONFIG_CONTENT
                .parse::<DocumentMut>()
                .map_err(|e| CoreError::Config(e.to_string()))?
        } else {
            content
                .parse::<DocumentMut>()
                .map_err(|e| CoreError::Config(e.to_string()))?
        };

        let skill_table = match doc.as_table_mut().entry("skill") {
            toml_edit::Entry::Occupied(mut occupied) => {
                if occupied.get().as_table().is_none() {
                    occupied.insert(Item::Table(TomlEditTable::new()));
                }
                occupied
                    .into_mut()
                    .as_table_mut()
                    .expect("skill entry is a table")
            }
            toml_edit::Entry::Vacant(vacant) => vacant
                .insert(Item::Table(TomlEditTable::new()))
                .as_table_mut()
                .expect("just inserted a table"),
        };

        let stale: Vec<String> = skill_table
            .iter()
            .map(|(key, _)| key.to_string())
            .filter(|key| !enabled.contains_key(key))
            .collect();
        for key in stale {
            skill_table.remove(&key);
        }

        for (key, value) in enabled {
            let item: Item = TomlEditValue::from(*value).into();
            match skill_table.entry(key) {
                toml_edit::Entry::Occupied(mut occupied) => {
                    occupied.insert(item);
                }
                toml_edit::Entry::Vacant(vacant) => {
                    vacant.insert(item);
                }
            }
        }

        std::fs::write(config_path, doc.to_string()).map_err(CoreError::from)?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod tests;
