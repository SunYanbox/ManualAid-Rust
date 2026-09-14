//! Tests for the `todo_write` tool.
//! `todo_write` 工具的测试。
//!
//! Every case drives `run_at` with a temporary root, so neither the process
//! working directory nor the real `.ManualAid` folder is ever touched.
//! 每个用例都以临时根驱动 `run_at`，因此既不触碰进程工作目录，也不触碰真实
//! 的 `.ManualAid` 目录。

use super::*;
use crate::todo;
use std::path::PathBuf;

/// Self-cleaning temporary workspace root.
/// 自清理的临时工作区根。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("manualaid-todo-write-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Create a plan file the call can link to.
    /// 建立调用可绑定的计划文件。
    fn write_plan(&self, name: &str) {
        let dir = todo::plans_dir(self.path());
        std::fs::create_dir_all(&dir).expect("create plans dir");
        std::fs::write(dir.join(format!("{name}.md")), "# plan\n").expect("write plan");
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Build the parameter map for one call.
/// 构造一次调用的参数映射。
fn params(subject: &Value, todos: Value) -> IndexMap<String, Value> {
    let mut map = IndexMap::new();
    map.insert("subject".to_string(), subject.clone());
    map.insert("todos".to_string(), todos);
    map
}

/// One task object.
/// 单个任务对象。
fn task(name: &str, status: &str) -> Value {
    serde_json::json!({ "task": name, "status": status })
}

#[tokio::test]
async fn create_without_the_flag_reports_not_created() {
    let root = TempRoot::new("not-created");
    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(result.success);
    assert_eq!(
        result.output,
        "The todo list with the subject \"alpha\" was not created."
    );
    assert!(!todo::todos_dir(root.path()).join("alpha.json").exists());
}

#[tokio::test]
async fn not_created_message_lists_similar_subjects() {
    let root = TempRoot::new("not-created-similar");
    let first = params(
        &serde_json::json!("abcdefghijklmnopqrst"),
        serde_json::json!([task("a", "pending")]),
    );
    let mut create = first.clone();
    create.insert("create".to_string(), Value::Bool(true));
    run_at(root.path(), &create).await;

    let map = params(
        &serde_json::json!("abcdefghijklmnopqrsZ"),
        serde_json::json!([task("a", "pending")]),
    );
    let result = run_at(root.path(), &map).await;

    assert!(result.output.contains("was not created"));
    assert!(result.output.contains("pass `create: true`"));
    assert!(result.output.contains("abcdefghijklmnopqrst"));
}

#[tokio::test]
async fn create_flag_writes_the_document() {
    let root = TempRoot::new("create-flag");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending"), task("b", "completed")]),
    );
    map.insert("create".to_string(), Value::Bool(true));

    let result = run_at(root.path(), &map).await;

    assert!(result.success);
    assert_eq!(
        result.output,
        "Updated the todo list with the subject \"alpha\": 1 pending, 0 in progress, 1 completed."
    );

    let stored = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");
    assert_eq!(stored.todos.len(), 2);
    assert!(!stored.create_datetime.is_empty());
    assert_eq!(stored.create_datetime, stored.update_datetime);
}

#[tokio::test]
async fn overwriting_preserves_creation_time_and_refreshes_update_time() {
    let root = TempRoot::new("overwrite");
    let mut create = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    create.insert("create".to_string(), Value::Bool(true));
    run_at(root.path(), &create).await;

    let first = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");

    let update = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "completed")]),
    );
    run_at(root.path(), &update).await;

    let second = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");

    assert_eq!(first.create_datetime, second.create_datetime);
    assert_eq!(second.todos.len(), 1);
    assert!(second.todos[0].status == TodoStatus::Completed);
}

#[tokio::test]
async fn an_existing_list_ignores_a_missing_create_flag() {
    let root = TempRoot::new("existing-no-create");
    let mut create = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    create.insert("create".to_string(), Value::Bool(true));
    run_at(root.path(), &create).await;

    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "in_progress")]),
    );
    let result = run_at(root.path(), &map).await;

    assert!(result.success);
    assert!(result.output.contains("1 in progress"));
}

#[tokio::test]
async fn blank_subject_is_rejected() {
    let root = TempRoot::new("blank-subject");
    let map = params(
        &serde_json::json!("   "),
        serde_json::json!([task("a", "pending")]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`subject`"));
}

#[tokio::test]
async fn missing_todos_is_rejected() {
    let root = TempRoot::new("missing-todos");
    let mut map = IndexMap::new();
    map.insert("subject".to_string(), serde_json::json!("alpha"));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`todos` is required"));
}

#[tokio::test]
async fn empty_todos_is_rejected() {
    let root = TempRoot::new("empty-todos");
    let map = params(&serde_json::json!("alpha"), serde_json::json!([]));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("at least one entry"));
}

#[tokio::test]
async fn non_array_todos_is_rejected() {
    let root = TempRoot::new("non-array-todos");
    let map = params(&serde_json::json!("alpha"), serde_json::json!("nope"));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("must be an array"));
}

#[tokio::test]
async fn non_object_entry_is_rejected_with_its_index() {
    let root = TempRoot::new("non-object-entry");
    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending"), 42]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`todos[1]`"));
}

#[tokio::test]
async fn blank_task_is_rejected_with_its_index() {
    let root = TempRoot::new("blank-task");
    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("  ", "pending")]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`todos[0].task`"));
}

#[tokio::test]
async fn unknown_status_is_rejected_with_its_index() {
    let root = TempRoot::new("unknown-status");
    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "doing")]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`todos[0].status`"));
    assert!(result.output.contains("doing"));
}

#[tokio::test]
async fn missing_status_is_rejected() {
    let root = TempRoot::new("missing-status");
    let map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([{ "task": "a" }]),
    );

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("`todos[0].status`"));
}

#[tokio::test]
async fn linked_plan_with_a_separator_is_rejected() {
    let root = TempRoot::new("plan-separator");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    map.insert("linked_plan".to_string(), serde_json::json!("nested/plan"));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("single file name"));
}

#[tokio::test]
async fn linked_plan_that_does_not_exist_is_rejected() {
    let root = TempRoot::new("plan-missing");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    map.insert("linked_plan".to_string(), serde_json::json!("ghost"));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("does not exist"));
}

#[tokio::test]
async fn a_valid_linked_plan_is_persisted() {
    let root = TempRoot::new("plan-persist");
    root.write_plan("my-plan");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    map.insert("linked_plan".to_string(), serde_json::json!("my-plan"));
    map.insert("create".to_string(), Value::Bool(true));

    run_at(root.path(), &map).await;

    let stored = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");
    assert_eq!(stored.linked_plan.as_deref(), Some("my-plan"));
}

#[tokio::test]
async fn omitting_linked_plan_keeps_the_existing_binding() {
    let root = TempRoot::new("plan-keep");
    root.write_plan("my-plan");
    let mut first = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    first.insert("linked_plan".to_string(), serde_json::json!("my-plan"));
    first.insert("create".to_string(), Value::Bool(true));
    run_at(root.path(), &first).await;

    let second = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "in_progress")]),
    );
    run_at(root.path(), &second).await;

    let stored = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");
    assert_eq!(stored.linked_plan.as_deref(), Some("my-plan"));
}

#[tokio::test]
async fn completing_every_task_archives_the_list_and_its_plan() {
    let root = TempRoot::new("archive");
    root.write_plan("my-plan");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "completed"), task("b", "completed")]),
    );
    map.insert("linked_plan".to_string(), serde_json::json!("my-plan"));
    map.insert("create".to_string(), Value::Bool(true));

    let result = run_at(root.path(), &map).await;

    assert!(result.success);
    assert!(
        result
            .output
            .contains("All tasks in the subject \"alpha\" are complete")
    );
    assert!(result.output.contains("The todo list has been archived to"));
    assert!(
        result
            .output
            .contains("The linked plan file has been archived to")
    );
    assert!(!todo::todos_dir(root.path()).join("alpha.json").exists());
    assert!(!todo::plans_dir(root.path()).join("my-plan.md").exists());
}

#[tokio::test]
async fn completing_without_a_linked_plan_does_not_archive() {
    let root = TempRoot::new("complete-no-plan");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "completed")]),
    );
    map.insert("create".to_string(), Value::Bool(true));

    let result = run_at(root.path(), &map).await;

    assert!(result.success);
    assert!(result.output.contains("1 completed"));
    assert!(todo::todos_dir(root.path()).join("alpha.json").exists());
}

#[tokio::test]
async fn a_blank_linked_plan_is_treated_as_absent() {
    let root = TempRoot::new("plan-blank");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    map.insert("linked_plan".to_string(), serde_json::json!("   "));
    map.insert("create".to_string(), Value::Bool(true));

    run_at(root.path(), &map).await;

    let stored = todo::load(root.path(), "alpha")
        .await
        .expect("load")
        .expect("exists");
    assert!(stored.linked_plan.is_none());
}

#[tokio::test]
async fn a_malformed_document_is_reported_instead_of_overwritten() {
    let root = TempRoot::new("malformed");
    let dir = todo::todos_dir(root.path());
    std::fs::create_dir_all(&dir).expect("create todos dir");
    std::fs::write(dir.join("alpha.json"), "{ not json").expect("write malformed");
    let mut map = params(
        &serde_json::json!("alpha"),
        serde_json::json!([task("a", "pending")]),
    );
    map.insert("create".to_string(), Value::Bool(true));

    let result = run_at(root.path(), &map).await;

    assert!(!result.success);
    assert!(result.output.contains("invalid todo JSON"));
}
