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
//! # Naming / 命名
//! Each loaded skill carries the stable, source-aware key
//! `<scope>-<agent_dir>-<name>` in [`Skill::unique_name`]: `scope` is
//! `project` or `global`, `<agent_dir>` is the `AGENT_DIRS` entry the skill
//! was found under (e.g. `.claude`), `<name>` is the frontmatter `name:` or
//! the folder name. A residual collision — two folders in the same scan root
//! declaring the same frontmatter `name` — is resolved with a deterministic
//! `-N` suffix during deduplication. The name an agent sees and passes to the
//! skill tool is the shortest form that stays unique among the *enabled*
//! skills (see [`exposed_name_map`]); management/config surfaces always show
//! the full stable key.
//! 每个已加载技能携带稳定且可追溯来源的键
//! `<scope>-<agent_dir>-<name>`，存放于 [`Skill::unique_name`]：`scope` 为
//! `project` 或 `global`；`<agent_dir>` 是发现技能所用的 `AGENT_DIRS`
//! 目录名（如 `.claude`）；`<name>` 为 frontmatter 的 `name:` 或目录名。
//! 同一扫描根内两个目录声明相同 frontmatter `name` 的残余冲突，在去重时
//! 以确定性 `-N` 后缀解决。代理可见并可传给 skill 工具的名称，是已启用
//! 技能中保持唯一的最短形式（见 [`exposed_name_map`]）；管理/配置界面
//! 始终显示完整稳定键。
//! # 测试说明
//! 扫描器中 `read_dir` / 条目遍历的错误分支以及非 UTF-8 文件夹名的跳过
//! 分支依赖测试无法跨平台构造的 OS 状态；这些分支不要求高测试覆盖率。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::file_io;
use crate::user_dir;

mod config_io;
mod frontmatter;
mod scan;

use config_io::{path_key, read_enabled_map, write_enabled_map};
use scan::{apply_enabled, dedup_skills, scan_skills_dir};

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
/// stable, source-aware key `<scope>-<agent_dir>-<name>` (e.g.
/// `project-.claude-greeter`) documented in the module documentation; the
/// name an agent passes to the skill tool is usually shorter and is computed
/// by [`exposed_name_map`].
/// # 描述
/// 技能由 [`reload_skills`] 从模块文档所述的项目与全局搜索目录加载。
/// `unique_name` 是稳定且可追溯来源的键 `<scope>-<agent_dir>-<name>`
/// （如 `project-.claude-greeter`），见模块文档；代理传给 skill 工具的名称
/// 通常更短，由 [`exposed_name_map`] 计算。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    /// Stable lookup key `<scope>-<agent_dir>-<name>`, unique per loaded
    /// skill; see the module documentation for the collision rules.
    /// 稳定查找键 `<scope>-<agent_dir>-<name>`，每个已加载技能唯一；冲突
    /// 规则见模块文档。
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
    /// Agent configuration directory the skill folder was found under,
    /// verbatim from `AGENT_DIRS` (e.g. `.claude`, `.agents`).
    /// 技能目录所在的 agent 配置目录，与 `AGENT_DIRS` 中的写法一致（如
    /// `.claude`、`.agents`）。
    pub agent_dir: String,
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

/// Map every skill in `skills` from its stable `unique_name` to the
/// agent-facing name: the shortest form that identifies the skill within
/// the given set, intended for the *enabled* skills (see [`enabled_skills`]).
///
/// A skill whose plain `name` is unique in the set maps to `name`; when
/// several skills share a name, each of them maps to `<agent_dir>-<name>`,
/// and only a group that still collides — the same agent directory holding
/// the same name in both scopes — maps to the full `unique_name`. Names
/// stay distinct even when a plain name coincides with another skill's
/// prefixed form.
/// 将 `skills` 中每个技能从稳定 `unique_name` 映射到代理可见名称：在给定
/// 集合内可唯一识别该技能的最短形式，意图用于*已启用*技能（见
/// [`enabled_skills`]）。
///
/// 某技能在集合内的裸 `name` 唯一时映射为 `name`；多个技能共享同一
/// `name` 时各自映射为 `<agent_dir>-<name>`，只有仍然冲突的组——同一
/// agent 目录下同一名称同时来自项目与全局范围——映射为完整
/// `unique_name`。即使某技能的裸名恰好等于另一技能的前缀形式，映射结果
/// 也保持互不相同。
pub fn exposed_name_map(skills: &[Skill]) -> HashMap<String, String> {
    let mut plain_counts: HashMap<&str, usize> = HashMap::new();
    for skill in skills {
        *plain_counts.entry(skill.name.as_str()).or_default() += 1;
    }
    let mut dir_counts: HashMap<(&str, &str), usize> = HashMap::new();
    for skill in skills.iter().filter(|s| plain_counts[s.name.as_str()] > 1) {
        *dir_counts
            .entry((skill.agent_dir.as_str(), skill.name.as_str()))
            .or_default() += 1;
    }

    let mut map: HashMap<String, String> = skills
        .iter()
        .map(|skill| {
            let exposed = if plain_counts[skill.name.as_str()] == 1 {
                skill.name.clone()
            } else if dir_counts[&(skill.agent_dir.as_str(), skill.name.as_str())] == 1 {
                format!("{}-{}", skill.agent_dir, skill.name)
            } else {
                skill.unique_name.clone()
            };
            (skill.unique_name.clone(), exposed)
        })
        .collect();

    // A plain name can coincide with another skill's prefixed exposed name
    // (e.g. a skill literally named `.agents-pdf` next to a colliding
    // `.agents` group); fall back to the stable name for every holder of a
    // duplicated exposed name so the mapping stays one-to-one.
    // 某技能的裸名可能与另一技能的前缀暴露名相同（例如名为 `.agents-pdf`
    // 的技能紧邻一组冲突的 `.agents` 技能）；对持有重复暴露名的技能全部
    // 回退到稳定名，保证映射一一对应。
    let mut exposed_counts: HashMap<&str, usize> = HashMap::new();
    for exposed in map.values() {
        *exposed_counts.entry(exposed).or_default() += 1;
    }
    if exposed_counts.values().any(|count| *count > 1) {
        let duplicated: Vec<String> = map
            .iter()
            .filter(|(_, exposed)| exposed_counts[exposed.as_str()] > 1)
            .map(|(unique, _)| unique.clone())
            .collect();
        for unique in duplicated {
            let fallback = unique.clone();
            map.insert(unique, fallback);
        }
    }
    map
}

/// Resolve an agent-supplied skill name to the loaded skill it refers to.
///
/// Resolution order:
/// 1. the stable `unique_name` of any loaded skill — disabled skills match
///    too, so callers can tell "disabled by the user" apart from "not
///    found";
/// 2. the exposed name of an enabled skill (see [`exposed_name_map`]);
/// 3. a plain `name` matching exactly one loaded skill, enabled or not.
///
/// Anything else — in particular a plain name shared by several loaded
/// skills — returns `None`.
/// 将代理提供的技能名解析为对应的已加载技能。
///
/// 解析顺序：
/// 1. 任意已加载技能的稳定 `unique_name` —— 禁用技能同样可匹配，调用方
///    能区分“已被用户禁用”与“未找到”；
/// 2. 已启用技能的暴露名（见 [`exposed_name_map`]）；
/// 3. 恰好匹配一个已加载技能（无论是否启用）的裸 `name`。
///
/// 其余任何情况——尤其是被多个已加载技能共享的裸名——返回 `None`。
pub fn resolve_skill(agent_name: &str) -> Option<Skill> {
    resolve_in(&read_store().skills, agent_name)
}

/// Pure variant of [`resolve_skill`] over an explicit skill slice.
/// [`resolve_skill`] 的纯函数变体，作用于显式技能切片。
fn resolve_in(skills: &[Skill], agent_name: &str) -> Option<Skill> {
    if let Some(skill) = skills.iter().find(|s| s.unique_name == agent_name) {
        return Some(skill.clone());
    }
    let enabled: Vec<Skill> = skills.iter().filter(|s| s.is_enabled).cloned().collect();
    let exposed = exposed_name_map(&enabled);
    if let Some(skill) = skills.iter().find(|s| {
        s.is_enabled && exposed.get(&s.unique_name).map(String::as_str) == Some(agent_name)
    }) {
        return Some(skill.clone());
    }
    let mut plain_matches = skills.iter().filter(|s| s.name == agent_name);
    let first = plain_matches.next()?;
    if plain_matches.next().is_some() {
        return None;
    }
    Some(first.clone())
}

/// Return the loaded skill whose stable unique name matches exactly, or
/// `None`. Prefer [`resolve_skill`] for agent-supplied names.
/// 返回稳定唯一名称精确匹配的已加载技能，未找到时返回 `None`。代理提供的
/// 名称请优先使用 [`resolve_skill`]。
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
            found.extend(scan_skills_dir(&search_dir, agent_dir, false)?);
        }
    }
    for agent_dir in AGENT_DIRS {
        let search_dir = home.join(agent_dir).join("skills");
        if search_dir.is_dir() {
            found.extend(scan_skills_dir(&search_dir, agent_dir, true)?);
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

#[cfg(test)]
mod tests;
