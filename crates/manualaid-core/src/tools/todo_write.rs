//! The `todo_write` tool: the model submits a full TODO list each call and
//! the tool owns persistence, statistics and archiving.
//! `todo_write` 工具：模型每次调用提交完整 TODO 列表，由工具负责持久化、
//! 统计与归档。
//!
//! No parameter carries path semantics on purpose. The document location is
//! derived from `subject` alone, so a call never needs an approval prompt and
//! can never reach outside `<root>/.ManualAid/todos/`.
//! 刻意不给任何参数路径语义：文档位置仅由 `subject` 推导，因此调用永不触发
//! 审批，也永远无法写到 `<root>/.ManualAid/todos/` 之外。

use std::path::Path;

use indexmap::IndexMap;
use serde_json::Value;

use super::tool::ToolResult;
use super::{get_bool, get_string};
use crate::todo::{self, TodoItem, TodoList, TodoStatus};

/// Canonical tool name, also used in every result message.
/// 规范工具名，同时用于所有结果消息。
const NAME: &str = "todo_write";

/// Execute `todo_write` against the process working directory.
/// 以进程工作目录为根执行 `todo_write`。
pub async fn run(params: &IndexMap<String, Value>) -> ToolResult {
    let root = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(error) => {
            return ToolResult::failure(
                NAME,
                format!("cannot resolve the working directory: {error}"),
            );
        }
    };
    run_at(&root, params).await
}

/// Execute `todo_write` against an explicit workspace root.
/// 以显式指定的工作区根执行 `todo_write`。
pub async fn run_at(root: &Path, params: &IndexMap<String, Value>) -> ToolResult {
    let subject = get_string(params, "subject").unwrap_or_default();
    let subject = subject.trim();
    if subject.is_empty() {
        return ToolResult::failure(NAME, "`subject` must be a non-empty string");
    }

    let Some(todos_value) = params.get("todos") else {
        return ToolResult::failure(NAME, "`todos` is required");
    };
    let todos = match parse_todos(todos_value) {
        Ok(todos) => todos,
        Err(message) => return ToolResult::failure(NAME, message),
    };

    let linked_plan = get_string(params, "linked_plan")
        .map(|plan| plan.trim().to_string())
        .filter(|plan| !plan.is_empty());

    if let Some(plan) = &linked_plan {
        if !todo::is_plan_name_valid(plan) {
            return ToolResult::failure(
                NAME,
                format!(
                    "`linked_plan` must be a single file name without path separators, got `{plan}`"
                ),
            );
        }
        if !todo::is_plan_valid(root, plan) {
            return ToolResult::failure(
                NAME,
                format!(
                    "`linked_plan` points at `{}`, which does not exist under `.ManualAid/plans/`",
                    plan
                ),
            );
        }
    }

    let existing = match todo::load(root, subject).await {
        Ok(list) => list,
        Err(error) => return ToolResult::failure(NAME, error.to_string()),
    };

    let mut list = match existing {
        Some(list) => list,
        None if !get_bool(params, "create").unwrap_or(false) => {
            return ToolResult::success(NAME, not_created_message(root, subject), false);
        }
        None => TodoList {
            subject: subject.to_string(),
            create_datetime: todo::now_rfc3339(),
            update_datetime: String::new(),
            linked_plan: None,
            todos: Vec::new(),
        },
    };

    // A call that omits `linked_plan` keeps whatever the list already had, so
    // a model can refresh task states without repeating the binding.
    // 未提供 `linked_plan` 的调用保留列表原有的绑定，使模型可以只刷新任务状态
    // 而不必重复声明绑定关系。
    if let Some(plan) = linked_plan {
        list.linked_plan = Some(plan);
    }
    list.todos = todos;
    list.update_datetime = todo::now_rfc3339();

    if let Err(error) = todo::save(root, &list).await {
        return ToolResult::failure(NAME, error.to_string());
    }

    let ready_to_archive = todo::is_complete(&list)
        && list
            .linked_plan
            .as_deref()
            .is_some_and(|plan| todo::is_plan_valid(root, plan));
    if ready_to_archive {
        return match todo::archive(root, &list).await {
            Ok(outcome) => ToolResult::success(NAME, archived_message(&list, &outcome), false),
            Err(error) => ToolResult::failure(NAME, error),
        };
    }

    let (pending, in_progress, completed) = todo::counts(&list);
    ToolResult::success(
        NAME,
        format!(
            "Updated the todo list with the subject \"{subject}\": {pending} pending, {in_progress} in progress, {completed} completed."
        ),
        false,
    )
}

/// Parse and validate the `todos` array.
///
/// Element shape is checked here rather than by the executor: the shared type
/// check only verifies that the value is an array, never its members.
/// 解析并校验 `todos` 数组。
///
/// 元素结构在此校验而非由执行器负责：共享的类型检查只确认值是数组，从不检查
/// 其成员。
fn parse_todos(value: &Value) -> Result<Vec<TodoItem>, String> {
    let array = value
        .as_array()
        .ok_or_else(|| "`todos` must be an array".to_string())?;
    if array.is_empty() {
        return Err("`todos` must contain at least one entry".to_string());
    }

    array
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let object = entry
                .as_object()
                .ok_or_else(|| format!("`todos[{index}]` must be an object"))?;
            let task = object
                .get("task")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|task| !task.is_empty())
                .ok_or_else(|| format!("`todos[{index}].task` must be a non-empty string"))?;
            let status = match object.get("status").and_then(Value::as_str) {
                Some("pending") => TodoStatus::Pending,
                Some("in_progress") => TodoStatus::InProgress,
                Some("completed") => TodoStatus::Completed,
                Some(other) => {
                    return Err(format!(
                        "`todos[{index}].status` must be `pending`, `in_progress` or `completed`, got `{other}`"
                    ));
                }
                None => {
                    return Err(format!(
                        "`todos[{index}].status` must be `pending`, `in_progress` or `completed`"
                    ));
                }
            };
            Ok(TodoItem {
                task: task.to_string(),
                status,
            })
        })
        .collect()
}

/// Build the message for a subject that does not exist yet.
///
/// Near-duplicate subjects are listed so the model can reuse an existing list
/// instead of silently creating a second one that differs by a word.
/// 构造主题尚不存在时的消息。
///
/// 会列出高度相似的主题，使模型可以复用既有列表，而不是悄悄创建只差一个词的
/// 第二份。
fn not_created_message(root: &Path, subject: &str) -> String {
    let mut out = format!("The todo list with the subject \"{subject}\" was not created.");
    let similar = todo::similar_subjects(root, subject);
    if similar.is_empty() {
        return out;
    }

    out.push_str("\nSimilar existing subjects are shown below; pass `create: true` to create a new list, or reuse one of them:");
    for candidate in similar {
        out.push('\n');
        out.push_str(&candidate);
    }
    out
}

/// Build the message for a list that was just archived.
/// 构造刚刚归档的列表所对应的消息。
fn archived_message(list: &TodoList, outcome: &todo::ArchiveOutcome) -> String {
    let mut out = format!(
        "All tasks in the subject \"{}\" are complete. The todo list has been archived to {}.",
        list.subject,
        outcome.todo_path.display()
    );
    if let Some(plan_path) = &outcome.plan_path {
        out.push_str(&format!(
            " The linked plan file has been archived to {}.",
            plan_path.display()
        ));
    }
    out
}

#[cfg(test)]
#[path = "todo_write_tests.rs"]
mod tests;
