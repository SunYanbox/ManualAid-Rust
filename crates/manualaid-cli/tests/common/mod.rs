//! Shared helpers for integration tests: a unique temp directory removed on
//! drop, and a helper that writes a skill folder.
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
            "manualaid-cli-test-{}-{}-{tag}",
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

/// Run the compiled binary in `cwd` with a scripted stdin and captured
/// output. `home` is forwarded to the child when set so the loop never
/// touches the real user home. The child's stdio is redirected, so the
/// interactive pager and clear-screen paths stay quiet and assertions can
/// inspect the real console output.
/// 在 `cwd` 中以脚本化 stdin 运行编译后的二进制并捕获输出。设置 `home` 时转发
/// 给子进程，避免 loop 触碰真实用户主目录。子进程 stdio 被重定向，交互分页与
/// 清屏路径保持安静，断言可以直接检查真实控制台输出。
pub fn run_binary_scripted(
    cwd: &Path,
    home: Option<&Path>,
    args: &[&str],
    lines: &[&str],
) -> std::process::Output {
    use std::io::Write;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_manualaid-cli"));
    command
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if let Some(home) = home {
        command.env("HOME", home).env("USERPROFILE", home);
    }
    let mut child = command.spawn().expect("spawn manualaid-cli binary");
    {
        let mut stdin = child.stdin.take().expect("child stdin handle");
        for line in lines {
            writeln!(stdin, "{line}").expect("write scripted input");
        }
    }
    child
        .wait_with_output()
        .expect("wait for manualaid-cli binary")
}

/// Write a skill folder `root/agent_dir/skills/{folder}/SKILL.md` with the
/// given frontmatter fields, returning the skill folder path. `name` is
/// omitted from the frontmatter when `None`.
/// 以给定的 frontmatter 字段写入技能文件夹
/// `root/agent_dir/skills/{folder}/SKILL.md`，返回技能文件夹路径。`name`
/// 为 `None` 时 frontmatter 中不含 `name:` 字段。
pub fn write_skill(
    root: &Path,
    agent_dir: &str,
    folder: &str,
    name: Option<&str>,
    description: &str,
) -> PathBuf {
    let dir = root.join(agent_dir).join("skills").join(folder);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    let mut frontmatter = String::from("---\n");
    if let Some(name) = name {
        frontmatter.push_str(&format!("name: {name}\n"));
    }
    frontmatter.push_str(&format!("description: {description}\n---\n"));
    std::fs::write(dir.join("SKILL.md"), frontmatter).expect("write SKILL.md");
    dir
}
