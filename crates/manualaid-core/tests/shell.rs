// Tests serialize against each other with a std Mutex held across awaits;
// the guard is never re-entered, so the lint does not apply here.
// 测试用 std Mutex 跨 await 串行化互斥；守卫不会被重入，此 lint 不适用。
#![allow(clippy::await_holding_lock)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use manualaid_core::error::CoreError;
use manualaid_core::shell::{
    CommandResult, reset_shell_path, run_program, run_shell, set_shell_path, shell_path,
};

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
    let result = run_shell(command, Some(Duration::from_millis(500)))
        .await
        .expect("timeout is not an error");
    assert!(result.timed_out);
    assert!(
        start.elapsed() < Duration::from_secs(2),
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
        start.elapsed() < Duration::from_secs(2),
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
    // 2s timeout: PowerShell startup is slow, and the 5s sleep gives the kill
    // plenty of room, but the assertion about "started" needs the engine up.
    #[cfg(windows)]
    let result = run_program(
        "powershell.exe",
        &[
            "-NoProfile",
            "-Command",
            "Write-Output started; Start-Sleep -Seconds 5",
        ],
        Some(Duration::from_secs(2)),
    )
    .await;
    #[cfg(not(windows))]
    let result = run_program(
        "/bin/sh",
        &["-c", "echo started; sleep 5"],
        Some(Duration::from_millis(500)),
    )
    .await;
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

/// With no shell configured, the default falls back to `cmd` when `%COMSPEC%`
/// is unset (and to `sh` when `$SHELL` is unset).
/// 未配置 Shell 且 `%COMSPEC%`（Unix 为 `$SHELL`）缺失时，默认回退到
/// `cmd`（Unix 为 `sh`）。
#[tokio::test]
async fn default_shell_falls_back_without_env() {
    let _guard = lock_shell();
    let _restore = ShellRestore { old: shell_path() };
    reset_shell_path();
    #[cfg(windows)]
    {
        let original = std::env::var_os("COMSPEC");
        // SAFETY: the shell lock serializes this test against every other
        // test touching the shell path, and none of them read COMSPEC while
        // it is unset. The value is restored right after the run.
        unsafe { std::env::remove_var("COMSPEC") };
        let result = run_shell("echo fallback", None)
            .await
            .expect("cmd fallback should work");
        if let Some(value) = original {
            // SAFETY: same reasoning as the removal above.
            unsafe { std::env::set_var("COMSPEC", value) };
        }
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "fallback");
    }
    #[cfg(not(windows))]
    {
        let original = std::env::var_os("SHELL");
        // SAFETY: same reasoning as the Windows branch above.
        unsafe { std::env::remove_var("SHELL") };
        let result = run_shell("echo fallback", None)
            .await
            .expect("sh fallback should work");
        if let Some(value) = original {
            // SAFETY: same reasoning as the removal above.
            unsafe { std::env::set_var("SHELL", value) };
        }
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout.trim(), "fallback");
    }
}

/// On GBK systems (ANSI codepage 936), cmd output decodes to UTF-8.
/// 在 GBK 系统（ANSI 代码页 936）上，cmd 输出可正确解码为 UTF-8。
#[cfg(windows)]
#[tokio::test]
async fn run_shell_decodes_gbk_output() {
    let _guard = lock_shell();
    let probe = run_program(
        "powershell.exe",
        &[
            "-NoProfile",
            "-Command",
            "[System.Text.Encoding]::Default.CodePage",
        ],
        None,
    )
    .await
    .expect("codepage probe should run");
    if !probe.stdout.contains("936") {
        return;
    }
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
