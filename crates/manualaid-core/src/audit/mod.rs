//! The auditor inspects tool-call parameters through their
//! [`ParamSemantic`](crate::tools::ParamSemantic) tags and decides whether
//! an operation is safe, needs user approval, or should be denied — without
//! hard-coding tool names.
//! 审计器通过参数的 [`ParamSemantic`](crate::tools::ParamSemantic) 标签
//! 检查工具调用，并决定操作是安全、需要用户批准还是应被拒绝——不依赖
//! 工具名硬编码。
//!
//! # Decision matrix
//! | Semantic | Condition | Decision |
//! |----------|-----------|----------|
//! | `ReadPath` | inside workspace | Allowed |
//! | `WritePath` | inside workspace | Manual → NeedsApproval; AcceptEdit → Allowed |
//! | `ReadPath` / `WritePath` | outside workspace | NeedsApproval |
//! | `Command` | on whitelist | Allowed |
//! | `Command` | not on whitelist | NeedsApproval (dangerous patterns are Denied) |
//! | `Content` | — | Allowed (privacy handled by the sanitize/restore pipeline) |
//! | `None` | — | Allowed |

pub mod strategies;

use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tools::{ParamSemantic, ToolKind};

/// Operating mode that affects audit behaviour.
/// 影响审计行为的操作模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionMode {
    /// Every write/edit operation requires user approval.
    /// 每次写入/编辑操作都需要用户批准。
    #[default]
    Manual,
    /// Write/edit operations inside the workspace auto-approve;
    /// outside-workspace operations still require approval.
    /// 工作区内的写入/编辑操作自动放行；工作区外的操作仍需批准。
    AcceptEdit,
}

/// The outcome of auditing a single parameter.
/// 审计单个参数的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditDecision {
    /// The operation is allowed without user interaction.
    /// 操作无需用户交互即被允许。
    Allowed,
    /// The operation is denied — it should not proceed.
    /// 操作被拒绝——不应继续执行。
    Denied(String),
    /// User approval must be obtained before proceeding.
    /// 继续执行前必须获得用户批准。
    NeedsApproval(String),
}

impl AuditDecision {
    /// Returns `true` when the decision is [`Allowed`](AuditDecision::Allowed).
    /// 当决策为 [`Allowed`](AuditDecision::Allowed) 时返回 `true`。
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// Returns a human-readable reason, or `None` for `Allowed`.
    /// 返回人类可读的原因，或在对 [`Allowed`](AuditDecision::Allowed) 时返回 `None`。
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Allowed => None,
            Self::Denied(reason) | Self::NeedsApproval(reason) => Some(reason),
        }
    }
}

/// An item waiting in the audit approval queue.
/// 等待审计批准队列中的项目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditQueueItem {
    /// Tool name that triggered the audit.
    /// 触发审计的工具名称。
    pub tool_name: String,
    /// Parameter name that triggered the audit.
    /// 触发审计的参数名称。
    pub param_name: String,
    /// Decision returned.
    /// 返回的决策结果。
    pub decision: AuditDecision,
}

/// Performs permission and safety audits on tool-call parameters.
/// 对工具调用参数执行权限和安全审计。
pub struct Auditor {
    /// Workspace root for path-boundary checks.
    /// 用于路径边界检查的工作区根目录。
    pub(crate) workspace_root: PathBuf,
    /// Pre-approved shell commands (exact or base-command match).
    /// 预批准的 shell 命令（精确匹配或基础命令匹配）。
    pub(crate) allowed_commands: Vec<String>,
    /// Additional paths exempt from workspace-boundary checks.
    /// 免除工作区边界检查的额外路径。
    pub(crate) exempt_paths: Vec<PathBuf>,
    /// Session operating mode (Manual / AcceptEdit).
    /// 会话操作模式（Manual / AcceptEdit）。
    pub(crate) mode: SessionMode,
}

impl Auditor {
    /// Create a new auditor rooted at `workspace_root`.
    /// 创建一个以 `workspace_root` 为根的新审计器。
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            allowed_commands: Vec::new(),
            exempt_paths: Vec::new(),
            mode: SessionMode::default(),
        }
    }

    /// Configure allowed (whitelisted) commands.
    /// 配置允许（白名单）的命令。
    pub fn with_allowed_commands(mut self, commands: Vec<String>) -> Self {
        self.allowed_commands = commands;
        self
    }

    /// Configure exempt paths.
    /// 配置免除路径。
    pub fn with_exempt_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.exempt_paths = paths;
        self
    }

    /// Configure the session operating mode (Manual / AcceptEdit).
    /// 配置会话操作模式（Manual / AcceptEdit）。
    pub fn with_mode(mut self, mode: SessionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Audit every parameter of a tool call against its tool definition.
    ///
    /// Returns `(param_name, decision)` pairs; only parameters that
    /// triggered a non-Allowed decision are included. An empty vec means
    /// everything passed.
    /// 根据工具定义对工具调用的每个参数进行审计。
    ///
    /// 返回 `(param_name, decision)` 对；仅包含触发了非 Allowed 决策的
    /// 参数。空的 vec 表示所有参数均通过。
    pub fn check(
        &self,
        params: &IndexMap<String, Value>,
        tool: ToolKind,
    ) -> Vec<(String, AuditDecision)> {
        let mut results = Vec::new();
        for param in tool.parameters() {
            let value = match params.get(param.name) {
                Some(Value::String(value)) => value.clone(),
                Some(other) => other.to_string(),
                None => continue,
            };
            let decision = match param.semantic {
                ParamSemantic::None => AuditDecision::Allowed,
                ParamSemantic::ReadPath | ParamSemantic::WritePath => {
                    self.check_path(&value, param.semantic)
                }
                ParamSemantic::Command => self.check_command(&value),
                ParamSemantic::Content => self.check_content(&value),
            };
            if !decision.is_allowed() {
                results.push((param.name.to_string(), decision));
            }
        }
        results
    }

    /// Given audit results from one or more tool calls, produce a flat
    /// sequence of items that need user approval, in presentation order.
    /// 根据一个或多个工具调用的审计结果，生成需要用户批准的扁平项目
    /// 序列（按展示顺序）。
    pub fn build_approval_queue(
        results: &[(String, Vec<(String, AuditDecision)>)],
    ) -> Vec<AuditQueueItem> {
        let mut queue = Vec::new();
        for (tool_name, decisions) in results {
            for (param_name, decision) in decisions {
                if matches!(decision, AuditDecision::NeedsApproval(_)) {
                    queue.push(AuditQueueItem {
                        tool_name: tool_name.clone(),
                        param_name: param_name.clone(),
                        decision: decision.clone(),
                    });
                }
            }
        }
        queue
    }
}
