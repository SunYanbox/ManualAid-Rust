//! Content audit strategy: privacy protection lives in the sanitize/restore
//! pipeline, so `Content` parameters are unconditionally allowed.
//! Content 审计策略：隐私保护由脱敏/还原管线负责，因此 `Content` 参数
//! 无条件放行。

use crate::audit::AuditDecision;

impl crate::audit::Auditor {
    /// Check a `Content` semantic parameter. Always allowed — sensitive
    /// data filtering is handled by the prompt sanitisation pipeline.
    /// 检查 `Content` 语义参数。始终放行——敏感数据过滤由提示净化管线
    /// 处理。
    pub(crate) fn check_content(&self, _content: &str) -> AuditDecision {
        AuditDecision::Allowed
    }
}
