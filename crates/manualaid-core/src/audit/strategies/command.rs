//! Command audit strategy: whitelist matching plus hard denial of known
//! dangerous patterns.
//! 命令审计策略：白名单匹配，并对已知危险模式硬拒绝。

use crate::audit::AuditDecision;

/// Concrete commands that no whitelist entry may ever match. `mkfs.` and
/// the fork bomb are represented as real commands so wildcard patterns such
/// as `mkfs*` or `*(){*` are caught alongside exact entries like `rm -rf /`.
/// 任何白名单条目都不允许匹配的具体命令。`mkfs.` 与 fork 炸弹以真实命令
/// 表示，使 `mkfs*`、`*(){*` 这类通配符模式与 `rm -rf /` 这类精确条目
/// 一样能被拦截。
const BLACKLISTED_COMMANDS: &[&str] = &[
    "rm -rf /",
    "> /dev/sda",
    "mkfs.ext4 /dev/sda",
    ":(){ :|:& };:",
];

/// Whether `pattern` matches `text` under whitelist wildcard rules:
/// `*` matches zero or more arbitrary characters, everything else matches
/// literally.
/// 判断 `pattern` 是否按白名单通配符规则匹配 `text`：`*` 匹配零个或多个
/// 任意字符，其余字符按字面匹配。
///
/// # Description
/// The matcher walks the pattern and text with a two-row dynamic program
/// where `dp[j]` means the pattern prefix processed so far matches
/// `text[..j]`. A `*` may consume nothing (carry `dp[j]`) or one more
/// character (carry `next[j - 1]`).
/// # 描述
/// 匹配器用两行动态规划遍历模式与文本，`dp[j]` 表示已处理的模式前缀
/// 匹配 `text[..j]`。`*` 可以不消费字符（继承 `dp[j]`），也可以再消费
/// 一个字符（继承 `next[j - 1]`）。
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let mut dp = vec![false; text.len() + 1];
    dp[0] = true;
    for &pc in &pattern {
        let mut next = vec![false; text.len() + 1];
        if pc == '*' {
            for j in 0..=text.len() {
                next[j] = dp[j] || (j > 0 && next[j - 1]);
            }
        } else {
            for j in 0..text.len() {
                if dp[j] && text[j] == pc {
                    next[j + 1] = true;
                }
            }
        }
        dp = next;
    }
    dp[text.len()]
}

/// Whether a whitelist entry can match a blacklisted command. Exact
/// entries (`rm -rf /`) and wildcard entries (`rm *`, `*`) that match any
/// blacklisted command are too dangerous to keep.
/// 白名单条目是否可能命中黑名单命令。精确条目（`rm -rf /`）与通配符条目
/// （`rm *`、`*`）一旦匹配任一黑名单命令，就过于危险，不应保留。
pub fn is_dangerous_allow_command(pattern: &str) -> bool {
    BLACKLISTED_COMMANDS
        .iter()
        .any(|command| wildcard_match(pattern, command))
}

/// Split a user-configured whitelist into commands safe to keep and
/// commands matching the blacklist that must be ignored. Order is
/// preserved so diagnostics stay stable.
/// 将用户配置的白名单拆分为可保留的命令与命中黑名单、必须忽略的命令。
/// 顺序保持不变，保证诊断信息稳定。
pub fn sanitize_allow_commands(commands: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut kept = Vec::new();
    let mut ignored = Vec::new();
    for command in commands {
        if is_dangerous_allow_command(&command) {
            ignored.push(command);
        } else {
            kept.push(command);
        }
    }
    (kept, ignored)
}

impl crate::audit::Auditor {
    /// Check a `Command` semantic parameter against the whitelist.
    /// 根据命令白名单检查 `Command` 语义参数。
    pub(crate) fn check_command(&self, command: &str) -> AuditDecision {
        let command = command.trim();

        // Known dangerous patterns are denied first so a broad whitelist
        // entry (e.g. a base command like `rm`) can never re-enable them.
        const DANGEROUS_KEYWORDS: &[&str] = &["rm -rf /", "> /dev/sda", "mkfs.", ":(){ :|:& };:"];
        if DANGEROUS_KEYWORDS
            .iter()
            .any(|pattern| command.contains(pattern))
        {
            return AuditDecision::Denied(
                i18n::t_str("audit.command.dangerous").replace("%{command}", command),
            );
        }

        // Chaining detection must run against the full command, not the
        // base command, otherwise glued chain characters (`echo;rm -rf /`)
        // or whitespace-split newlines would bypass the check.
        // 链接检测必须作用于完整命令而非基础命令，否则粘连的链接字符
        // （如 `echo;rm -rf /`）或被空白切分的换行会绕过检测。
        const CHAINING_CHARS: &[char] = &[';', '&', '|', '`', '\n', '\r'];
        let has_chaining = command.contains(CHAINING_CHARS) || command.contains("$(");

        // A command containing chain characters can never be whitelisted,
        // otherwise `git *` would auto-allow `git status; rm -rf /`.
        // 含链接字符的命令永远不能命中白名单，否则 `git *` 会自动放行
        // `git status; rm -rf /`。
        let whitelisted = !has_chaining
            && self
                .allowed_commands
                .iter()
                .any(|allowed| wildcard_match(allowed, command));

        if whitelisted {
            return AuditDecision::Allowed;
        }

        AuditDecision::NeedsApproval(
            i18n::t_str("audit.command.whitelist").replace("%{command}", command),
        )
    }
}
