//! Shared helpers for integration tests: a unique temp directory that is
//! removed on drop, and a helper that writes a skill folder.
//! 集成测试的共享辅助：Drop 时移除的唯一临时目录，以及写入技能文件夹的
//! 辅助函数。
//!
//! Every test binary includes this module but not every helper is used by
//! every binary, so dead-code warnings are suppressed here.
//! 每个测试二进制都会引入本模块，但并非每个辅助函数都会被用到，
//! 因此在此抑制 dead-code 警告。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A unique temporary directory removed on drop.
/// 移除时清理的唯一临时目录。
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Create a new unique temp directory for a test.
    /// 为测试创建新的唯一临时目录。
    pub fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "manualaid-test-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    /// The temp directory path.
    /// 临时目录路径。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Write a skill folder `agent_dir/skills/{folder}/SKILL.md` with the given
/// frontmatter fields and body, returning the skill folder path. `name` is
/// omitted from the frontmatter when `None`.
/// 以给定的 frontmatter 字段与正文写入技能文件夹
/// `agent_dir/skills/{folder}/SKILL.md`，返回技能文件夹路径。`name` 为
/// `None` 时 frontmatter 中不含 `name:` 字段。
pub fn write_skill(
    agent_dir: &Path,
    folder: &str,
    name: Option<&str>,
    description: &str,
    body: &str,
) -> PathBuf {
    let dir = agent_dir.join("skills").join(folder);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    let mut frontmatter = String::from("---\n");
    if let Some(name) = name {
        frontmatter.push_str(&format!("name: {name}\n"));
    }
    frontmatter.push_str(&format!("description: {description}\n---\n"));
    std::fs::write(dir.join("SKILL.md"), format!("{frontmatter}{body}")).expect("write SKILL.md");
    dir
}
