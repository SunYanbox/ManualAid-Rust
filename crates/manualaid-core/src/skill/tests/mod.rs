use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::config_io::{path_key, read_enabled_map, write_enabled_map};
use super::frontmatter::{Frontmatter, parse_frontmatter};
use super::scan::{apply_enabled, dedup_skills, stable_unique_name};
use super::*;

mod config_io;
mod frontmatter;
mod resolve;
mod scan;
mod store;

/// Build a skill with the stable unique name the scanner would assign for
/// `agent_dir` and `is_global`; enabled by default for project skills.
/// 构造技能，其唯一名称与扫描器对 `agent_dir`、`is_global` 所赋的一致；
/// 项目技能默认启用。
fn skill(
    name: &str,
    description: &str,
    body: &str,
    agent_dir: &str,
    path: &str,
    is_global: bool,
) -> Skill {
    Skill {
        unique_name: stable_unique_name(is_global, agent_dir, name),
        name: name.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        path: PathBuf::from(path),
        agent_dir: agent_dir.to_string(),
        is_global,
        is_enabled: !is_global,
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "manualaid-skill-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}
