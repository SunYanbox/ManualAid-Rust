use std::path::Path;

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
