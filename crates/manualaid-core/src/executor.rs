//! The executor pipeline: route → validate → restore masked placeholders →
//! audit → run → post-process.
//! 执行器管线：路由 → 参数校验 → 还原掩码占位符 → 审计 → 执行 →
//! 结果后处理。

use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use crate::audit::{AuditDecision, Auditor};
use crate::parser::ParsedToolCall;
use crate::privacy::{restore_masked_data, sanitize_prompt};
use crate::tools::{ToolKind, ToolParam, ToolResult, params_summary_of};

/// Errors that can occur during tool-call execution.
/// 工具调用执行期间可能发生的错误。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutorError {
    /// Human-readable description.
    /// 人类可读的错误描述。
    pub message: String,
    /// Name of the tool that caused the error.
    /// 导致错误的工具名称。
    pub tool_name: String,
}

impl ExecutorError {
    /// Create a new executor error.
    /// 创建一个新的执行器错误。
    pub fn new(message: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            tool_name: tool_name.into(),
        }
    }
}

impl std::fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.tool_name, self.message)
    }
}

impl std::error::Error for ExecutorError {}

/// The central execution pipeline.
/// 中央执行管线。
pub struct Executor {
    /// Permission/safety checks.
    /// 权限/安全检查。
    auditor: Auditor,
    /// Reversible placeholder mapping produced by prompt sanitisation,
    /// restored on string parameters before execution and re-applied to
    /// results afterwards.
    /// 提示净化产生的可逆占位符映射：执行前在字符串参数上还原，执行后
    /// 对结果重新应用。
    prompt_mapping: Arc<Option<HashMap<String, String>>>,
}

impl Executor {
    /// Build an executor from its components.
    /// 从组件构建执行器。
    pub fn new(auditor: Auditor, prompt_mapping: Arc<Option<HashMap<String, String>>>) -> Self {
        Self {
            auditor,
            prompt_mapping,
        }
    }

    /// Run a single parsed tool call through the full pipeline.
    ///
    /// Errors at any stage are folded into the returned [`ToolResult`]; the
    /// method itself is infallible.
    /// 通过完整管线执行单个解析后的工具调用。
    ///
    /// 任何阶段的错误都会被折叠到返回的 [`ToolResult`] 中（该方法本身
    /// 不会失败）。
    pub async fn execute(&self, call: ParsedToolCall) -> ToolResult {
        let tool_name = &call.tool_name;

        let Some(tool) = ToolKind::from_name(tool_name) else {
            return ToolResult::failure(
                tool_name,
                format!("Unknown tool `{tool_name}` — not in the available-tools list"),
            )
            .with_params_summary(params_summary_of(&call.params));
        };

        let coerced_params = coerce_params(&call.params, tool);
        if let Err(error) = validate_params(&coerced_params, tool) {
            return ToolResult::failure(tool_name, error.message)
                .with_params_summary(params_summary_of(&call.params));
        }

        let audit_results = self.auditor.check(&coerced_params, tool);
        let hard_denials: Vec<&str> = audit_results
            .iter()
            .filter_map(|(param, decision)| {
                matches!(decision, AuditDecision::Denied(_)).then_some(param.as_str())
            })
            .collect();
        if !hard_denials.is_empty() {
            return ToolResult::failure(
                tool_name,
                format!(
                    "Operation denied by audit for parameter(s): {}",
                    hard_denials.join(", ")
                ),
            )
            .with_params_summary(params_summary_of(&call.params));
        }

        let restored_params = restore_params(&coerced_params, &self.prompt_mapping);
        let restored_call = ParsedToolCall {
            params: restored_params,
            ..call
        };

        let exec_start = std::time::Instant::now();
        let mut result = tool.run(&restored_call.params).await;
        result.execution_duration_ms = exec_start.elapsed().as_millis() as u64;
        result.audit_decisions = audit_results;
        self.post_process(&mut result, tool);
        result.with_params_summary(params_summary_of(&call.params))
    }

    /// Audit a parsed tool call without executing it, so the caller can
    /// build an approval queue before any side effects occur.
    /// 审计解析后的工具调用而不执行它，使调用方能在任何副作用发生前
    /// 构建审批队列。
    pub fn audit(&self, call: &ParsedToolCall) -> Vec<(String, AuditDecision)> {
        let Some(tool) = ToolKind::from_name(&call.tool_name) else {
            return Vec::new();
        };
        let coerced_params = coerce_params(&call.params, tool);
        self.auditor.check(&coerced_params, tool)
    }

    /// Pre-check a parsed tool call for guaranteed failure without
    /// executing it. Returns `Some(failure result)` when the call is
    /// guaranteed to fail and `None` when it should proceed to the
    /// approval queue.
    /// 预检解析后的工具调用，判断其是否必然失败，而不执行它。调用必然
    /// 失败时返回 `Some(失败结果)`，可正常推进时返回 `None`。
    pub async fn pre_check(&self, call: &ParsedToolCall) -> Option<ToolResult> {
        let tool = ToolKind::from_name(&call.tool_name)?;

        let coerced_params = coerce_params(&call.params, tool);
        if let Err(error) = validate_params(&coerced_params, tool) {
            return Some(
                ToolResult::failure(&call.tool_name, error.message)
                    .with_params_summary(params_summary_of(&call.params)),
            );
        }

        let restored_params = restore_params(&coerced_params, &self.prompt_mapping);
        if let Err(message) = match tool {
            ToolKind::Edit => crate::tools::edit::plan_edit(&restored_params)
                .await
                .map(|_| ()),
            ToolKind::Read => crate::tools::read::pre_check(&restored_params).await,
            _ => Ok(()),
        } {
            return Some(
                ToolResult::failure(&call.tool_name, message)
                    .with_params_summary(params_summary_of(&call.params)),
            );
        }

        None
    }

    /// Look up a tool kind by name.
    /// 按名称查找工具。
    pub fn find_tool(&self, name: &str) -> Option<ToolKind> {
        ToolKind::from_name(name)
    }

    /// Return the number of built-in tools.
    /// 返回内置工具的数量。
    pub fn tool_count(&self) -> usize {
        crate::tools::all_tools().len()
    }

    /// Post-process a result: empty-output substitution and privacy
    /// re-sanitisation.
    /// 后处理结果：空输出替换与隐私重新净化。
    fn post_process(&self, result: &mut ToolResult, tool: ToolKind) {
        if result.output.trim().is_empty() && result.success {
            result.output =
                i18n::t_str("cli.output.empty_output").replace("%{tool_name}", tool.name());
            result.is_fallback = true;
        }

        if let Some(mapping) = self.prompt_mapping.as_ref()
            && !result.output.trim().is_empty()
        {
            let restored = restore_masked_data(&result.output, mapping);
            if let Ok((clean, _)) = sanitize_prompt(&restored) {
                result.output = clean;
            }
        }
    }
}

/// Restore masked placeholders on every string parameter.
/// 在每个字符串参数上还原掩码占位符。
fn restore_params(
    params: &IndexMap<String, Value>,
    prompt_mapping: &Arc<Option<HashMap<String, String>>>,
) -> IndexMap<String, Value> {
    let mut restored = params.clone();
    if let Some(mapping) = prompt_mapping.as_ref() {
        for value in restored.values_mut() {
            if let Value::String(text) = value {
                *value = Value::String(restore_masked_data(text, mapping));
            }
        }
    }
    restored
}

/// Validate `params` against a tool's parameter descriptors: required-ness
/// and value types.
/// 根据工具的参数描述符验证 `params`：必要性检查与类型检查。
///
/// # Errors
/// Returns an [`ExecutorError`] for the first violation found.
/// # 错误
/// 在发现第一个违规时返回 [`ExecutorError`]。
fn validate_params(params: &IndexMap<String, Value>, tool: ToolKind) -> Result<(), ExecutorError> {
    let descriptors = tool.parameters();
    let desc_map: HashMap<&str, &ToolParam> = descriptors
        .iter()
        .map(|param| (param.name, param))
        .collect();

    for descriptor in &descriptors {
        if !descriptor.required {
            continue;
        }
        match params.get(descriptor.name) {
            None => {
                return Err(ExecutorError::new(
                    format!("Missing required parameter `{}`", descriptor.name),
                    tool.name(),
                ));
            }
            Some(Value::String(value)) if value.is_empty() => {
                return Err(ExecutorError::new(
                    format!("Required parameter `{}` must not be empty", descriptor.name),
                    tool.name(),
                ));
            }
            Some(Value::Null) => {
                return Err(ExecutorError::new(
                    format!("Required parameter `{}` must not be null", descriptor.name),
                    tool.name(),
                ));
            }
            _ => {}
        }
    }

    for (key, value) in params {
        if let Some(descriptor) = desc_map.get(key.as_str())
            && let Err(message) = check_value_type(descriptor.name, descriptor.kind, value)
        {
            return Err(ExecutorError::new(message, tool.name()));
        }
    }

    Ok(())
}

/// Coerce string values into their declared JSON types so type validation
/// and execution see values consistent with the descriptor.
/// 将字符串值按声明类型转换为对应的 JSON 类型，使类型校验与执行拿到的
/// 值与描述符一致。
fn coerce_params(params: &IndexMap<String, Value>, tool: ToolKind) -> IndexMap<String, Value> {
    let descriptors = tool.parameters();
    let desc_map: HashMap<&str, &ToolParam> = descriptors
        .iter()
        .map(|param| (param.name, param))
        .collect();

    let mut coerced = params.clone();
    for (key, value) in coerced.iter_mut() {
        let Value::String(text) = value else {
            continue;
        };
        if let Some(descriptor) = desc_map.get(key.as_str())
            && let Some(converted) = coerce_string(descriptor.kind, text)
        {
            *value = converted;
        }
    }
    coerced
}

/// Convert one string by declared kind; unparseable values stay unchanged.
/// 将单个字符串按声明类型转换；无法解析的值保持不变。
fn coerce_string(kind: &str, text: &str) -> Option<Value> {
    let trimmed = text.trim();
    match kind {
        "integer" => trimmed.parse::<i64>().ok().map(Value::from).or_else(|| {
            trimmed
                .parse::<f64>()
                .ok()
                .filter(|f| {
                    f.is_finite()
                        && f.fract() == 0.0
                        && *f >= i64::MIN as f64
                        && *f <= i64::MAX as f64
                })
                .map(|f| Value::from(f as i64))
        }),
        "number" | "float" | "double" => trimmed
            .parse::<f64>()
            .ok()
            .filter(|f| f.is_finite())
            .map(Value::from),
        "boolean" => match trimmed.to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        "object" => serde_json::from_str::<Value>(trimmed)
            .ok()
            .filter(Value::is_object),
        kind if kind.starts_with("array") => serde_json::from_str::<Value>(trimmed)
            .ok()
            .filter(Value::is_array),
        _ => None,
    }
}

/// Check a single value's JSON type against the declared kind.
/// 检查单个值的 JSON 类型是否与声明的 `kind` 匹配。
fn check_value_type(param_name: &str, kind: &str, value: &Value) -> Result<(), String> {
    let compatible = match kind {
        "string" => value.is_string(),
        "integer" => {
            value.is_i64()
                || value.is_u64()
                || value.as_f64().map(|f| f.fract() == 0.0).unwrap_or(false)
        }
        "number" | "float" | "double" => value.is_number(),
        "boolean" => value.is_boolean(),
        "array" | "array[string]" | "array[integer]" => value.is_array(),
        "object" => value.is_object(),
        // Untyped / generic — accept anything.
        _ => true,
    };

    if compatible {
        Ok(())
    } else {
        Err(format!(
            "Parameter `{param_name}` expected type `{kind}`, got `{}`",
            json_type_name(value)
        ))
    }
}

/// Human-readable name of a JSON value's runtime type.
/// JSON 值运行时类型的人类可读名称。
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "float"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests;
