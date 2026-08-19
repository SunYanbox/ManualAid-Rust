use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::config_io::{path_key, read_enabled_map, write_enabled_map};
use super::frontmatter::{Frontmatter, parse_frontmatter};
use super::scan::{apply_enabled, dedup_skills};
use super::*;

mod config_io;
mod frontmatter;
mod scan;
mod store;

fn skill(name: &str, description: &str, body: &str, path: &str, is_global: bool) -> Skill {
    Skill {
        unique_name: name.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        body: body.to_string(),
        path: PathBuf::from(path),
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
