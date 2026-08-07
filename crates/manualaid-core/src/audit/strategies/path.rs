//! Path audit strategy: workspace-boundary and exempt-list checks for
//! `ReadPath` / `WritePath` parameters.
//! 路径审计策略：对 `ReadPath` / `WritePath` 参数做工作区边界与豁免
//! 列表检查。

use std::path::Path;

use crate::audit::{AuditDecision, SessionMode};
use crate::tools::ParamSemantic;
use crate::workspace::{is_exempt_path, is_within_workspace, normalize_path};

impl crate::audit::Auditor {
    /// Check a `ReadPath` / `WritePath` semantic parameter.
    /// 检查 `ReadPath` / `WritePath` 语义参数。
    pub(crate) fn check_path(&self, path_str: &str, semantic: ParamSemantic) -> AuditDecision {
        let path = Path::new(path_str);
        let resolved = if path.is_relative() {
            self.workspace_root.join(path)
        } else {
            path.to_path_buf()
        };
        let normalised = normalize_path(&resolved);

        if is_within_workspace(&normalised, &self.workspace_root) {
            if semantic.is_write() && self.mode == SessionMode::Manual {
                AuditDecision::NeedsApproval(i18n::t_str("audit.path.write_manual"))
            } else {
                AuditDecision::Allowed
            }
        } else if is_exempt_path(&normalised, &self.exempt_paths) {
            AuditDecision::NeedsApproval(i18n::t_str("audit.path.exempt"))
        } else {
            AuditDecision::NeedsApproval(
                i18n::t_str("audit.path.outside").replace("%{path}", path_str),
            )
        }
    }
}
