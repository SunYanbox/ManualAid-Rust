// Tests serialize against each other with a std Mutex held across awaits;
// the guard is never re-entered, so the lint does not apply here.
// 测试用 std Mutex 跨 await 串行化互斥；守卫不会被重入，此 lint 不适用。
#![allow(clippy::await_holding_lock)]

use std::ffi::OsString;
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use manualaid_core::error::CoreError;
use manualaid_core::shell::{
    CommandResult, reset_shell_path, run_program, run_shell, set_shell_path, shell_path,
};

/// Timeout used by shell tests that must kill a long-running command.
/// 用于必须中止长时运行命令的 Shell 测试超时。
const TIMEOUT: Duration = Duration::from_millis(500);

/// Upper bound for timeout and bounded-drain tests to return. Must stay well
/// below the command's remaining sleep so the timeout path is deterministic.
/// 超时与有界排空测试返回的上界，需远小于命令剩余睡眠时间以保证确定性。
const MAX_RETURN_LATENCY: Duration = Duration::from_secs(2);

/// Serializes tests that touch the shared shell path static, because
/// `#[tokio::test]` bodies run concurrently.
/// 串行化触及共享 Shell 路径静态变量的测试（`#[tokio::test]` 会并发运行）。
static SHELL_LOCK: Mutex<()> = Mutex::new(());

fn lock_shell() -> MutexGuard<'static, ()> {
    SHELL_LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Restores the shell path to its pre-test value on drop. Declared after the
/// lock guard so the restore happens before the lock is released.
/// 析构时将 Shell 路径恢复为测试前的值。在锁守卫之后声明，确保先恢复后解锁。
struct ShellRestore {
    old: Option<PathBuf>,
}

impl Drop for ShellRestore {
    fn drop(&mut self) {
        match &self.old {
            Some(path) => {
                let _ = set_shell_path(path);
            }
            None => reset_shell_path(),
        }
    }
}

/// Restores one environment variable to its original value on drop, or
/// removes it again when it was originally unset.
/// 析构时将单个环境变量恢复为原值；原本不存在则再次移除。
struct EnvRestore {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvRestore {
    fn remove(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        // SAFETY: the shell lock serializes this test against every other
        // test reading this variable, and none of them observe the unset
        // state while the guard is alive.
        unsafe { std::env::remove_var(key) };
        Self { key, original }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => {
                // SAFETY: same reasoning as in remove().
                unsafe { std::env::set_var(self.key, value) };
            }
            None => {
                // SAFETY: same reasoning as in remove().
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
}

/// `run_shell` runs a command with the default shell and reports exit code 0.
/// `run_shell` 用默认 Shell 运行命令，退出码为 0。
#[tokio::test]
async fn run_shell_echo() {
    let _guard = lock_shell();
    let result = run_shell("echo hello", None)
        .await
        .expect("echo should run");
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "hello");
    assert_eq!(result.stderr.trim(), "");
}

/// A command finishing before the timeout is not marked timed out.
/// 在超时前正常结束的命令不会被标记为超时。
#[tokio::test]
async fn run_shell_finishes_within_timeout() {
    let _guard = lock_shell();
    let result = run_shell("echo hi", Some(Duration::from_secs(5)))
        .await
        .expect("echo should run");
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "hi");
}

/// `CommandResult` renders a readable summary of the execution.
/// `CommandResult` 渲染可读的执行摘要。
#[test]
fn command_result_display() {
    let result = CommandResult {
        stdout: "out".to_string(),
        stderr: "err".to_string(),
        exit_code: Some(0),
        signal: None,
        timed_out: false,
    };
    let text = result.to_string();
    assert!(text.contains("exit_code=0"));
    assert!(text.contains("out"));
    assert!(text.contains("err"));

    let timed_out = CommandResult {
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
        signal: Some(9),
        timed_out: true,
    };
    let text = timed_out.to_string();
    assert!(text.contains("timed out"));
    assert!(text.contains("exit_code=none"));
    assert!(text.contains("signal=9"));
}

/// stdout and stderr are captured separately.
/// stdout 和 stderr 被分别捕获。
#[tokio::test]
async fn run_shell_separates_stdout_and_stderr() {
    let _guard = lock_shell();
    #[cfg(windows)]
    let command = "echo out & echo err 1>&2";
    #[cfg(not(windows))]
    let command = "echo out; echo err >&2";
    let result = run_shell(command, None).await.expect("command should run");
    assert_eq!(result.exit_code, Some(0));
    assert!(result.stdout.trim().contains("out"));
    assert!(!result.stdout.contains("err"));
    assert!(result.stderr.trim().contains("err"));
}

/// A non-zero exit code is a normal result, not an error.
/// 非零退出码是正常结果而非错误。
#[tokio::test]
async fn run_shell_nonzero_exit_is_a_result() {
    let _guard = lock_shell();
    #[cfg(windows)]
    let command = "exit /b 3";
    #[cfg(not(windows))]
    let command = "exit 3";
    let result = run_shell(command, None)
        .await
        .expect("non-zero exit is a result");
    assert_eq!(result.exit_code, Some(3));
    assert!(!result.timed_out);
}

/// A timeout kills the command and still returns the output collected before
/// the kill.
/// 超时中止命令，并仍返回 kill 前收集到的输出。
#[tokio::test]
async fn run_shell_timeout_kills_and_preserves_output() {
    let _guard = lock_shell();
    #[cfg(windows)]
    let command = "echo started & ping -n 6 127.0.0.1";
    #[cfg(not(windows))]
    let command = "echo started; sleep 5";
    let start = Instant::now();
    let result = run_shell(command, Some(TIMEOUT))
        .await
        .expect("timeout is not an error");
    assert!(result.timed_out);
    assert!(
        start.elapsed() < MAX_RETURN_LATENCY,
        "timeout should return quickly, took {:?}",
        start.elapsed()
    );
    assert!(result.stdout.contains("started"));
}

/// Output from background grandchildren is drained for at most `DRAIN_GRACE`,
/// so a command that spawned orphans cannot hang the call.
/// 后台孙进程的输出排空至多 `DRAIN_GRACE`，产生孤儿进程的命令不会挂起调用。
#[tokio::test]
async fn drain_is_bounded_when_grandchildren_hold_pipes() {
    let _guard = lock_shell();
    #[cfg(windows)]
    let command = "start /b ping -n 6 127.0.0.1";
    #[cfg(not(windows))]
    let command = "sleep 3 &";
    let start = Instant::now();
    let result = run_shell(command, None)
        .await
        .expect("command should return");
    assert!(!result.timed_out);
    assert!(
        start.elapsed() < MAX_RETURN_LATENCY,
        "drain should be bounded, took {:?}",
        start.elapsed()
    );
}

/// `run_program` runs an explicit program with explicit arguments.
/// `run_program` 以显式参数运行指定的程序。
#[tokio::test]
async fn run_program_basic() {
    let _guard = lock_shell();
    #[cfg(windows)]
    let result = run_program("cmd", &["/C", "echo hello"], None).await;
    #[cfg(not(windows))]
    let result = run_program("/bin/sh", &["-c", "echo hello"], None).await;
    let result = result.expect("program should run");
    assert!(!result.timed_out);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "hello");
}

/// `run_program` applies the same timeout semantics as `run_shell`.
/// `run_program` 与 `run_shell` 采用相同的超时语义。
#[tokio::test]
async fn run_program_timeout_kills_and_preserves_output() {
    let _guard = lock_shell();
    // The Windows branch uses cmd: an immediate echo writes "started" before
    // the 500 ms timeout kills the process, and the ping keeps it running long
    // enough for the timeout to fire. PowerShell cold startup can exceed the
    // timeout on slow CI runners, which would lose the preserved output.
    // Windows 分支使用 cmd：echo 立即输出 "started"，ping 保证进程存活到
    // 超时触发；PowerShell 冷启动在慢 CI runner 上可能超过超时，导致
    // 输出保留断言失败。
    #[cfg(windows)]
    let result = run_program(
        "cmd.exe",
        &["/C", "echo started & ping -n 6 127.0.0.1"],
        Some(TIMEOUT),
    )
    .await;
    #[cfg(not(windows))]
    let result = run_program("/bin/sh", &["-c", "echo started; sleep 5"], Some(TIMEOUT)).await;
    let result = result.expect("timeout is not an error");
    assert!(result.timed_out);
    assert!(result.stdout.contains("started"));
}

/// A missing program surfaces as `CoreError::NotFound`.
/// 程序不存在时返回 `CoreError::NotFound`。
#[tokio::test]
async fn run_program_not_found_returns_not_found() {
    let result = run_program("definitely-not-a-real-program-xyz", &[], None).await;
    assert!(matches!(result, Err(CoreError::NotFound(_))));
}

/// An empty shell path is rejected up front.
/// 空 Shell 路径会被直接拒绝。
#[tokio::test]
async fn set_shell_path_rejects_empty() {
    let _guard = lock_shell();
    assert!(matches!(set_shell_path(""), Err(CoreError::InvalidPath(_))));
}

/// A nonexistent shell path is stored without validation and surfaces as
/// `CoreError::NotFound` when a command is spawned.
/// 不存在的 Shell 路径存储时不校验，spawn 命令时以 `CoreError::NotFound` 暴露。
#[tokio::test]
async fn set_shell_path_nonexistent_surfaces_not_found_at_run() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    set_shell_path("definitely-not-a-shell-xyz").expect("storing is lenient");
    let result = run_shell("echo hi", None).await;
    assert!(matches!(result, Err(CoreError::NotFound(_))));
}

/// `run_shell` uses the shell set by `set_shell_path` instead of the default.
/// `run_shell` 使用 `set_shell_path` 设置的 Shell 而非默认 Shell。
#[tokio::test]
async fn set_shell_path_changes_the_shell_used() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    #[cfg(windows)]
    {
        set_shell_path("powershell.exe").expect("store powershell");
        let result = run_shell("$PSVersionTable.PSVersion", None)
            .await
            .expect("powershell should run");
        assert!(
            result.stdout.contains("5"),
            "powershell version should be printed, got: {:?}",
            result.stdout
        );
    }
    #[cfg(not(windows))]
    {
        // echo ignores its arguments, so the command itself proves the path.
        set_shell_path("/bin/echo").expect("store echo");
        let result = run_shell("hello world", None)
            .await
            .expect("echo should run");
        assert!(result.stdout.contains("hello world"));
    }
}

/// `shell_path` reflects the configured value and `reset_shell_path` clears it.
/// `shell_path` 反映配置值，`reset_shell_path` 将其清空。
#[tokio::test]
async fn shell_path_getter_reflects_configuration() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    reset_shell_path();
    assert_eq!(shell_path(), None);
    set_shell_path("bash").expect("store bash");
    assert_eq!(shell_path(), Some(PathBuf::from("bash")));
    reset_shell_path();
    assert_eq!(shell_path(), None);
}

/// With no shell configured and `%COMSPEC%` unset, the default falls back to
/// `cmd`. The environment variable is restored by `EnvRestore` on drop.
/// 未配置 Shell 且 `%COMSPEC%` 缺失时，默认回退到 `cmd`；环境变量由
/// `EnvRestore` 在析构时恢复。
#[cfg(windows)]
#[tokio::test]
async fn default_shell_falls_back_without_env_windows() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    reset_shell_path();
    let _comspec = EnvRestore::remove("COMSPEC");
    let result = run_shell("echo fallback", None)
        .await
        .expect("cmd fallback should work");
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "fallback");
}

/// With no shell configured and `$SHELL` unset, the default falls back to
/// `sh`. The environment variable is restored by `EnvRestore` on drop.
/// 未配置 Shell 且 `$SHELL` 缺失时，默认回退到 `sh`；环境变量由
/// `EnvRestore` 在析构时恢复。
#[cfg(not(windows))]
#[tokio::test]
async fn default_shell_falls_back_without_env_unix() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    reset_shell_path();
    let _shell = EnvRestore::remove("SHELL");
    let result = run_shell("echo fallback", None)
        .await
        .expect("sh fallback should work");
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.stdout.trim(), "fallback");
}

/// On GBK systems (ANSI codepage 936), cmd output decodes to UTF-8.
/// 在 GBK 系统（ANSI 代码页 936）上，cmd 输出可正确解码为 UTF-8。
#[cfg(windows)]
#[tokio::test]
#[ignore = "requires ANSI codepage 936 (GBK)"]
async fn run_shell_decodes_gbk_output() {
    let _guard = lock_shell();
    let result = run_shell("echo 中文测试", None)
        .await
        .expect("echo should run");
    assert_eq!(result.exit_code, Some(0));
    assert!(
        result.stdout.contains("中文测试"),
        "GBK output should decode to UTF-8, got: {:?}",
        result.stdout
    );
}

/// Quoted paths reach git without literal quotes (issue #26 case 1).
/// 带引号的路径原样传给 git，不带字面引号（issue #26 案例 1）。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_passes_quoted_paths_verbatim_to_git() {
    let _guard = lock_shell();
    let probe = run_program("git", &["--version"], None)
        .await
        .expect("git probe should run");
    if probe.exit_code != Some(0) {
        eprintln!("skipping: git is not installed");
        return;
    }
    let result = run_shell("git add \"definitely-not-a-file-中文-路径.txt\"", None)
        .await
        .expect("git add should run");
    assert_eq!(result.exit_code, Some(128));
    assert!(
        result
            .stderr
            .contains("pathspec 'definitely-not-a-file-中文-路径.txt' did not match any files"),
        "pathspec should be clean, got: {:?}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("pathspec '\""),
        "pathspec must not carry literal quotes, got: {:?}",
        result.stderr
    );
}

/// A flag value wrapped in quotes is not word-split (issue #26 case 2).
/// 带引号的 flag 值不会被按词拆分（issue #26 案例 2）。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_passes_flag_with_quoted_value_to_git() {
    let _guard = lock_shell();
    let probe = run_program("git", &["--version"], None)
        .await
        .expect("git probe should run");
    if probe.exit_code != Some(0) {
        eprintln!("skipping: git is not installed");
        return;
    }
    let result = run_shell("git log --format=\"%h\" -1", None)
        .await
        .expect("git log should run");
    assert_eq!(result.exit_code, Some(0));
    let hash = result.stdout.trim();
    assert!(
        hash.len() >= 7 && hash.chars().all(|c| c.is_ascii_hexdigit()),
        "short hash expected, got: {:?}",
        result.stdout
    );
    assert!(
        !result.stdout.contains('"'),
        "format value must not carry quotes, got: {:?}",
        result.stdout
    );
}

/// An `&&` chain with quoted messages is not truncated (issue #26 case 3).
/// 含带引号消息的 `&&` 链不会被截断（issue #26 案例 3）。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_preserves_ampersand_chain_with_quoted_messages() {
    let _guard = lock_shell();
    let result = run_shell("echo \"first part\" && echo \"second part\"", None)
        .await
        .expect("chain should run");
    assert_eq!(result.exit_code, Some(0));
    assert!(
        result.stdout.contains("first part"),
        "got: {:?}",
        result.stdout
    );
    assert!(
        result.stdout.contains("second part"),
        "got: {:?}",
        result.stdout
    );
    assert!(
        !result.stdout.contains('\\'),
        "no backslash escapes expected, got: {:?}",
        result.stdout
    );
}

/// A quoted executable path as the first token keeps working: the extra
/// quote pair protects it from cmd /C's outer-quote stripping.
/// 首个 token 为带引号的可执行路径时仍可运行：额外引号对保护它不被
/// cmd /C 的外层引号剥离破坏。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_quoted_executable_first_token() {
    let _guard = lock_shell();
    if !Path::new(r"C:\Windows\System32\where.exe").exists() {
        eprintln!("skipping: where.exe is not present");
        return;
    }
    let result = run_shell("\"C:\\Windows\\System32\\where.exe\" where.exe", None)
        .await
        .expect("where should run");
    assert_eq!(result.exit_code, Some(0), "got: {:?}", result.stderr);
    assert!(
        result.stdout.to_lowercase().contains("where.exe"),
        "where should find itself, got: {:?}",
        result.stdout
    );
}

/// An empty command is a no-op: cmd /C with nothing exits 0 immediately.
/// 空命令是无操作：cmd /C 无参数时立即以 0 退出。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_empty_command_is_a_noop() {
    let _guard = lock_shell();
    let result = run_shell("", None).await.expect("empty command should run");
    assert_eq!(result.exit_code, Some(0));
    assert!(!result.timed_out);
}
