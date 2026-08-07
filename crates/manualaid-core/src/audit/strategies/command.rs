//! Command audit strategy: whitelist matching plus hard denial of known
//! dangerous patterns.
//! 命令审计策略：白名单匹配，并对已知危险模式硬拒绝。

use crate::audit::AuditDecision;

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
        let base_cmd = command.split_whitespace().next().unwrap_or(command);

        let whitelisted = self
            .allowed_commands
            .iter()
            .any(|allowed| allowed == command || (allowed == base_cmd && !has_chaining));

        if whitelisted {
            return AuditDecision::Allowed;
        }

        AuditDecision::NeedsApproval(
            i18n::t_str("audit.command.whitelist").replace("%{command}", command),
        )
    }
}
