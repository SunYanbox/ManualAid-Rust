//! Execute commands through a configurable shell, with optional timeout
//! termination.
//! 通过可配置的 Shell 执行命令，支持超时自动中止。
//!
//! # Description
//! The shell is selected by an explicit path set via
//! [`set_shell_path`], falling back to the platform default (`%COMSPEC%` on
//! Windows, `$SHELL` on Unix) when unset. All functions require a running
//! Tokio runtime. A timeout aborts the process but still returns every byte
//! of stdout and stderr collected so far.
//! # 描述
//! Shell 由 [`set_shell_path`] 显式设置，未设置时回退到平台默认
//! （Windows 的 `%COMSPEC%`、Unix 的 `$SHELL`）。所有函数必须在 Tokio
//! 运行时内调用。超时中止进程，但仍返回已收集的全部 stdout 和 stderr。

use chardetng::{Iso2022JpDetection, Utf8Detection};
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::borrow::Cow;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, Command as TokioCommand};

use crate::error::{CoreError, CoreResult};

/// The structured result of a shell command execution.
/// Shell 命令执行的结构化结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    /// Standard output (stdout) as a UTF-8 string.
    /// 标准输出（stdout），以 UTF-8 字符串形式返回。
    pub stdout: String,
    /// Standard error (stderr) as a UTF-8 string.
    /// 标准错误（stderr），以 UTF-8 字符串形式返回。
    pub stderr: String,
    /// Process exit code. `None` when the process was killed by a signal on
    /// Unix (e.g. after a timeout kill).
    /// 进程退出码。进程在 Unix 上被信号杀死时为 `None`（例如超时中止时）。
    #[serde(default)]
    pub exit_code: Option<i32>,
    /// On Unix, the signal that terminated the process (if killed by signal). `None` on Windows or when the process exited normally.
    /// 在 Unix 系统上，导致进程终止的信号（若进程被信号杀死）。在 Windows 上或进程正常退出时为 `None`。
    #[serde(default)]
    pub signal: Option<i32>,
    /// Whether the command was aborted by the timeout. The collected stdout
    /// and stderr remain fully available.
    /// 命令是否因超时被中止。已收集的 stdout 和 stderr 仍然完整可用。
    #[serde(default)]
    pub timed_out: bool,
}

impl std::fmt::Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.timed_out {
            write!(f, "timed out, ")?;
        }
        match self.exit_code {
            Some(code) => write!(f, "exit_code={code}")?,
            None => write!(f, "exit_code=none")?,
        }
        if let Some(sig) = self.signal {
            write!(f, ", signal={sig}")?;
        }
        write!(f, "\nstdout:\n{}\nstderr:\n{}", self.stdout, self.stderr)
    }
}

/// 使用自动编码检测将命令输出字节解码为 UTF-8。
///
/// 使用 `chardetng` 检测编码，再用 `encoding_rs` 的 Decoder 解码为 UTF-8。
/// 损坏的字节序列由 `encoding_rs` 替换为 Unicode 替换字符（U+FFFD），
/// 从而避免把 GBK 或 Shift_JIS 等非 UTF-8 编码的原始字节误当作 UTF-8。
pub(crate) fn decode_command_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    let mut detector = chardetng::EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(bytes, true);

    let encoding = detector.guess(None, Utf8Detection::Allow);
    // 使用 Decoder API 而非 `Encoding::decode`，避免依赖 encoding_rs 的
    // `alloc` feature（chardetng 以 default-features = false 引入 encoding_rs）。
    let mut decoder = encoding.new_decoder();
    let capacity = decoder
        .max_utf8_buffer_length(bytes.len())
        .unwrap_or_else(|| bytes.len().saturating_mul(3));
    let mut buf = vec![0u8; capacity];
    let (_result, _read, written, _had_errors) = decoder.decode_to_utf8(bytes, &mut buf, true);
    buf.truncate(written);
    // 解码器输出应为合法 UTF-8；from_utf8_lossy 仅作防御，兜底替换边界情形下
    // 可能残留的无效字节，不会把原始字节当作 UTF-8 解码。
    String::from_utf8_lossy(&buf).into_owned()
}

/// The currently configured shell path, or `None` when the platform default
/// is in use. Guarded by a mutex so the value can be updated at runtime.
/// 当前配置的 Shell 路径；`None` 表示使用平台默认值。用互斥锁保护以支持运行时更新。
static SHELL_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Run a command with the currently configured shell (or the platform
/// default when unset).
/// 使用当前配置的 Shell（未配置时使用平台默认）运行命令。
///
/// # Description
/// On Windows the shell is invoked with `/C`, on other
/// platforms with `-c`. A timeout aborts the process with `kill()` and still
/// returns every byte of stdout and stderr collected so far; the timeout is
/// reported through `CommandResult::timed_out` rather than an error. Note
/// that `kill()` only terminates the direct child — grandchildren (e.g.
/// `ping` started by `cmd`) may keep running and hold the pipes open, so the
/// pipes are drained for at most 500 ms after the process exits and the tail
/// of their output may be lost. Spawn failures and pipe errors are returned
/// as `CoreError`; a non-zero exit code is a normal result.
///
/// On Windows with a cmd shell the command is appended to the command line
/// verbatim (`raw_arg`): the standard argv escaping (`"` → `\"`) conflicts
/// with cmd's own `/C` quote parsing, which would pass literal quotes to
/// child programs. A command whose first non-whitespace character is `"` and
/// that also carries quoted arguments is wrapped in an extra quote pair so
/// cmd does not strip the outer quotes of a quoted executable path (with
/// exactly two quotes cmd preserves them itself). Non-cmd shells
/// (PowerShell) and non-Windows platforms keep standard argument escaping.
/// Like a real terminal, cmd splits multi-line commands into separate lines.
/// # 描述
/// Windows 上用 `/C`调用 Shell，其他平台用 `-c`。超时会用 `kill()` 中止进程，
/// 并仍返回已收集的全部 stdout 和 stderr；超时通过 `CommandResult::timed_out`
/// 报告而非错误。注意 `kill()` 只终止直接子进程——孙进程（例如 `cmd` 启动的 `ping`）
/// 可能短暂存活，kill 后最多再排空 500 ms。spawn 失败和管道错误返回 `CoreError`；
/// 非零退出码是正常结果。
///
/// Windows 下使用 cmd Shell 时，命令原样拼入命令行（`raw_arg`）：标准参数转义
/// （`"` → `\"`）与 cmd 自身的 `/C` 引号解析冲突，会把字面引号传给子进程。
/// 首个非空白字符为 `"` 且同时带引号参数的命令会额外包裹一对引号，避免 cmd
/// 剥离带引号可执行路径的外层引号（恰好两个引号时 cmd 会自己保留）。非 cmd
/// Shell（PowerShell）与非 Windows 平台保持标准参数转义。与真实终端一致，
/// cmd 会将多行命令按行拆分执行。
pub async fn run_shell(command: &str, timeout: Option<Duration>) -> CoreResult<CommandResult> {
    let shell = resolve_shell_path();
    let mut cmd = TokioCommand::new(&shell);
    cmd.args(shell_args(&shell))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    append_command_arg(&mut cmd, &shell, command);
    let mut child = cmd.spawn().map_err(CoreError::from)?;
    collect(&mut child, timeout).await
}

/// Run an arbitrary program with the given arguments.
/// 以给定参数运行任意程序。
///
/// # Description
/// Timeout semantics are identical to [`run_shell`]. On Windows,
/// batch files (`.bat`/`.cmd`) cannot be spawned directly; run them through a shell instead.
/// # 描述
/// 超时语义与 [`run_shell`] 相同。
/// 在 Windows 上，批处理文件（`.bat`/`.cmd`）无法直接 spawn，应通过 Shell 运行。
pub async fn run_program(
    program: impl AsRef<Path>,
    args: &[&str],
    timeout: Option<Duration>,
) -> CoreResult<CommandResult> {
    let mut cmd = TokioCommand::new(program.as_ref());
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(CoreError::from)?;
    collect(&mut child, timeout).await
}

/// Set the shell path used by [`run_shell`].
/// 设置 [`run_shell`] 使用的 Shell 路径。
///
/// # Description
/// The value is stored as-is without existence checks — a wrong path surfaces as
/// `CoreError::NotFound` when a command is spawned. An empty path is rejected
/// with `CoreError::InvalidPath`.
/// # 描述
/// 原样存储，不校验是否存在——错误的路径会在 spawn 命令时以
/// `CoreError::NotFound` 暴露。空路径以 `CoreError::InvalidPath` 拒绝。
pub fn set_shell_path(path: impl Into<PathBuf>) -> CoreResult<()> {
    let path = path.into();
    if path.as_os_str().is_empty() {
        return Err(CoreError::InvalidPath(
            "shell path must not be empty".to_string(),
        ));
    }
    *SHELL_PATH.lock().unwrap_or_else(PoisonError::into_inner) = Some(path);
    Ok(())
}

/// Return the currently configured shell path, or `None` when the platform
/// default is in use.
/// 返回当前配置的 Shell 路径；未配置（使用平台默认）时为 `None`。
pub fn shell_path() -> Option<PathBuf> {
    SHELL_PATH
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// The name of the currently effective shell (configured via
/// [`set_shell_path`] or the platform default), used by the CLI loop header.
/// 当前生效 Shell 的名称（由 [`set_shell_path`] 配置或平台默认），
/// 供 CLI loop 头部显示。
pub fn detected_shell() -> String {
    resolve_shell_path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| resolve_shell_path().to_string_lossy().into_owned())
}

/// Clear the configured shell path so the platform default is used again.
/// Hidden from docs because it exists for tests to restore state.
/// 清空配置的 Shell 路径，恢复使用平台默认。文档中隐藏，因为它供测试恢复状态用。
#[doc(hidden)]
pub fn reset_shell_path() {
    *SHELL_PATH.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// The configured shell, or the platform default when unset.
/// 配置的 Shell；未配置时返回平台默认。
fn resolve_shell_path() -> PathBuf {
    if let Some(path) = shell_path() {
        return path;
    }
    #[cfg(windows)]
    {
        std::env::var_os("COMSPEC")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("cmd"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("SHELL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("sh"))
    }
}

/// Whether the shell is Windows `cmd.exe` (or `command.com`), which is
/// invoked with `/C` and does not understand the standard `\"` argument
/// escaping.
/// 是否为 Windows 的 `cmd.exe`（或 `command.com`）Shell：以 `/C` 调用，
/// 且不理解标准的 `\"` 参数转义。
fn is_cmd_shell(shell: &Path) -> bool {
    let name = shell.file_name().and_then(OsStr::to_str).unwrap_or("");
    name.contains("cmd") || name == "command.com"
}

/// The argument used to run a command in the given shell: `/C` for cmd-like
/// shells, `-Command` for PowerShell, `-c` for everything else (sh, bash,
/// zsh, ...).
/// 给定 Shell 运行命令所用的参数：cmd 类 Shell 用 `/C`，PowerShell 用
/// `-Command`，其余（sh、bash、zsh 等）用 `-c`。
fn shell_args(shell: &Path) -> &'static [&'static str] {
    let name = shell.file_name().and_then(OsStr::to_str).unwrap_or("");
    if is_cmd_shell(shell) {
        &["/C"]
    } else if name.contains("powershell") || name.contains("pwsh") {
        &["-Command"]
    } else {
        &["-c"]
    }
}

/// Raw command-line text to append after the shell args: `Some` for cmd
/// shells (verbatim, since cmd `/C` does not understand `\"` escaping),
/// `None` for others (standard escaping is correct for shells that consume
/// arguments via `CommandLineToArgvW`, like PowerShell).
/// 追加到 Shell 参数之后的原始命令行文本：cmd Shell 返回 `Some`（原样传入，
/// 因为 cmd `/C` 不理解 `\"` 转义），其余返回 `None`（标准转义对通过
/// `CommandLineToArgvW` 接收参数的 Shell 如 PowerShell 是正确的）。
#[cfg(windows)]
fn cmd_raw_command<'a>(shell: &Path, command: &'a str) -> Option<Cow<'a, str>> {
    if !is_cmd_shell(shell) {
        return None;
    }
    if command.trim_start().starts_with('"') && command.matches('"').count() > 2 {
        // Wrap in an extra quote pair so cmd /C does not strip the outer
        // quotes of a quoted executable path (KB 830473). With exactly two
        // quotes cmd preserves them (KB rule 1), so only commands that also
        // carry quoted arguments need the wrap.
        // 额外包裹一对引号，避免 cmd /C 剥离带引号可执行路径的外层引号（KB 830473）。
        // 恰好两个引号时 cmd 会保留它们（KB 规则 1），因此只有同时带
        // 引号参数的命令需要包裹。
        Some(Cow::Owned(format!("\"\"{command}\"\"")))
    } else {
        Some(Cow::Borrowed(command))
    }
}

/// Append the command string to the shell invocation. On Windows with a cmd
/// shell the command is appended verbatim via `raw_arg` (cmd parses quotes
/// with its own rules, like a real terminal); everywhere else it is passed
/// as a normal argument with standard escaping.
/// 将命令字符串追加到 Shell 调用中。Windows 下使用 cmd Shell 时通过
/// `raw_arg` 原样追加（cmd 按自身规则解析引号，如同真实终端）；其余情况
/// 作为普通参数传入（标准转义）。
#[cfg(windows)]
fn append_command_arg(cmd: &mut TokioCommand, shell: &Path, command: &str) {
    match cmd_raw_command(shell, command) {
        Some(raw) => {
            cmd.raw_arg(raw.as_ref());
        }
        None => {
            cmd.arg(command);
        }
    }
}

#[cfg(not(windows))]
fn append_command_arg(cmd: &mut TokioCommand, _shell: &Path, command: &str) {
    cmd.arg(command);
}

/// How long to keep draining stdout/stderr after the child exits. A killed
/// or exited shell may leave grandchildren holding the pipe write ends open,
/// so the drain is bounded and whatever was collected so far is returned.
/// 子进程退出后继续排空 stdout/stderr 的时长。被 kill 或已退出的 Shell 可能
/// 留下持有管道写端的孙进程，因此排空有界，返回已收集的内容。
const DRAIN_GRACE: Duration = Duration::from_millis(500);

/// Read one chunk of up to `READ_CHUNK_SIZE` bytes into `buf`. Returns `Ok(0)`
/// at end of stream.
/// 读取最多 `READ_CHUNK_SIZE` 字节到 `buf`。流结束时返回 `Ok(0)`。
async fn read_chunk<S: AsyncRead + Unpin>(
    stream: &mut S,
    buf: &mut Vec<u8>,
) -> std::io::Result<usize> {
    let mut chunk = [0u8; 4096];
    let n = stream.read(&mut chunk).await?;
    buf.extend_from_slice(&chunk[..n]);
    Ok(n)
}

/// Wait for the child while concurrently draining stdout and stderr, so a
/// full pipe buffer can never deadlock the wait. On timeout the child is
/// killed and reaped. The final drain is bounded by `DRAIN_GRACE` in both
/// cases, so orphaned grandchildren holding the pipes cannot hang the call.
/// 等待子进程结束，同时并发排空 stdout 和 stderr，避免管道缓冲写满造成
/// 等待死锁。超时时 kill 并收割子进程。两种情况下的最终排空都由
/// `DRAIN_GRACE` 限制，持有管道写端的孤儿孙进程不会挂起调用。
async fn collect(child: &mut Child, timeout: Option<Duration>) -> CoreResult<CommandResult> {
    let mut stdout = child.stdout.take().expect("stdout must be piped");
    let mut stderr = child.stderr.take().expect("stderr must be piped");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut timed_out = false;

    let wait_and_drain = async {
        let mut status: Option<std::io::Result<std::process::ExitStatus>> = None;
        let (mut out_eof, mut err_eof) = (false, false);
        while status.is_none() {
            tokio::select! {
                s = child.wait() => status = Some(s),
                n = read_chunk(&mut stdout, &mut out), if !out_eof => {
                    let n = n?;
                    if n == 0 {
                        out_eof = true;
                    }
                }
                n = read_chunk(&mut stderr, &mut err), if !err_eof => {
                    let n = n?;
                    if n == 0 {
                        err_eof = true;
                    }
                }
            }
        }
        status.expect("the loop only exits when wait() returns")
    };
    let status = match timeout {
        Some(duration) => match tokio::time::timeout(duration, wait_and_drain).await {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => return Err(CoreError::from(e)),
            Err(_elapsed) => {
                timed_out = true;
                if let Err(e) = child.kill().await {
                    log::warn!("failed to kill child after timeout: {e}");
                }
                child.wait().await.map_err(CoreError::from)?
            }
        },
        None => wait_and_drain.await.map_err(CoreError::from)?,
    };

    let drain = async {
        let (mut out_eof, mut err_eof) = (false, false);
        while !(out_eof && err_eof) {
            tokio::select! {
                n = read_chunk(&mut stdout, &mut out), if !out_eof => {
                    let n = n?;
                    if n == 0 {
                        out_eof = true;
                    }
                }
                n = read_chunk(&mut stderr, &mut err), if !err_eof => {
                    let n = n?;
                    if n == 0 {
                        err_eof = true;
                    }
                }
            }
        }
        Ok::<(), std::io::Error>(())
    };
    // The final drain is bounded by `DRAIN_GRACE` in both the normal and the
    // timeout path: a shell that exited or was killed may leave grandchildren
    // holding the pipe write ends open, which would otherwise block forever.
    if let Ok(Err(e)) = tokio::time::timeout(DRAIN_GRACE, drain).await {
        return Err(CoreError::from(e));
    }

    #[cfg(unix)]
    let signal = std::os::unix::process::ExitStatusExt::signal(&status);
    #[cfg(not(unix))]
    let signal = None;

    Ok(CommandResult {
        stdout: decode_command_output(&out),
        stderr: decode_command_output(&err),
        exit_code: status.code(),
        signal,
        timed_out,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// is_cmd_shell recognizes cmd.exe and command.com by file name.
    /// is_cmd_shell 按文件名识别 cmd.exe 与 command.com。
    #[test]
    fn is_cmd_shell_detects_cmd_and_command_com() {
        // Absolute Windows paths only split on Windows; on Unix the
        // backslash is a regular filename character and the whole string
        // becomes the file name.
        // 绝对 Windows 路径仅在 Windows 上拆分；Unix 上反斜杠是普通字符，
        // 整个字符串成为文件名。
        #[cfg(windows)]
        let shells = [
            "cmd.exe",
            "C:\\Windows\\System32\\cmd.exe",
            "cmd",
            "command.com",
            "C:\\WINDOWS\\command.com",
        ];
        #[cfg(not(windows))]
        let shells = ["cmd.exe", "cmd", "command.com"];
        for shell in shells {
            assert!(is_cmd_shell(Path::new(shell)), "{shell}");
        }
    }

    /// is_cmd_shell rejects other shells.
    /// is_cmd_shell 拒绝其他 Shell。
    #[test]
    fn is_cmd_shell_rejects_other_shells() {
        for shell in [
            "powershell.exe",
            "pwsh.exe",
            "bash",
            "sh",
            "/bin/sh",
            "/usr/bin/zsh",
        ] {
            assert!(!is_cmd_shell(Path::new(shell)), "{shell}");
        }
    }

    /// A command run through a cmd shell is passed verbatim (no escaping).
    /// 经 cmd Shell 运行的命令原样传递（不做转义）。
    #[cfg(windows)]
    #[test]
    fn cmd_raw_command_passes_cmd_commands_verbatim() {
        let shell = Path::new("cmd.exe");
        assert_eq!(
            cmd_raw_command(shell, "echo hello").as_deref(),
            Some("echo hello")
        );
        assert_eq!(
            cmd_raw_command(shell, "echo one && echo \"two\"").as_deref(),
            Some("echo one && echo \"two\"")
        );
    }

    /// A command whose first non-whitespace character is a quote and that
    /// also carries quoted arguments is wrapped in an extra quote pair, so
    /// cmd /C keeps the quoted executable intact. With exactly two quotes
    /// cmd preserves them itself (KB 830473 rule 1), so the command is
    /// passed verbatim.
    /// 首个非空白字符为引号、且同时带引号参数的命令额外包裹一对引号，
    /// 避免 cmd /C 剥离带引号可执行路径的外层引号。恰好两个引号时 cmd
    /// 会自己保留它们（KB 830473 规则 1），命令原样传递。
    #[cfg(windows)]
    #[test]
    fn cmd_raw_command_wraps_quoted_first_token() {
        let shell = Path::new("cmd.exe");
        assert_eq!(
            cmd_raw_command(shell, r#""C:\Windows\System32\where.exe" where "a b""#).as_deref(),
            Some(r#""""C:\Windows\System32\where.exe" where "a b""""#)
        );
        assert_eq!(
            cmd_raw_command(shell, r#""C:\Windows\System32\where.exe" where.exe"#).as_deref(),
            Some(r#""C:\Windows\System32\where.exe" where.exe"#)
        );
        assert_eq!(
            cmd_raw_command(shell, r#"  "quoted" arg"#).as_deref(),
            Some(r#"  "quoted" arg"#)
        );
        assert_eq!(
            cmd_raw_command(shell, r#"echo "hi""#).as_deref(),
            Some(r#"echo "hi""#)
        );
    }

    /// Non-cmd shells keep standard argument escaping.
    /// 非 cmd Shell 保持标准参数转义。
    #[cfg(windows)]
    #[test]
    fn cmd_raw_command_is_none_for_non_cmd_shells() {
        assert_eq!(
            cmd_raw_command(Path::new("powershell.exe"), "echo hi"),
            None
        );
        assert_eq!(cmd_raw_command(Path::new("sh"), "echo hi"), None);
        assert_eq!(cmd_raw_command(Path::new("bash"), "echo hi"), None);
    }

    /// The non-Windows arm appends the command as a regular argument.
    /// 非 Windows 分支将命令作为普通参数追加。
    #[cfg(not(windows))]
    #[test]
    fn append_command_arg_non_windows_passthrough() {
        let mut cmd = TokioCommand::new("sh");
        append_command_arg(&mut cmd, Path::new("sh"), "echo hi");
        assert_eq!(cmd.as_std().get_program(), Path::new("sh"));
    }
}
