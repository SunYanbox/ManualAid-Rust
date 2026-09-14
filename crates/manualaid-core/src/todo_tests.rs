//! Tests for the TODO persistence module.
//! TODO 持久化模块的测试。
//!
//! Every case builds its own directory under the system temp dir and removes
//! it on drop, so the real `~/.ManualAid` and the workspace are never touched.
//! 每个用例都在系统临时目录下建立独立目录并在析构时删除，绝不触碰真实的
//! `~/.ManualAid` 与工作区。

use super::*;

/// Self-cleaning temporary root.
/// 自清理的临时根目录。
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("manualaid-todo-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Create a plan file the list can link to.
    /// 建立列表可绑定的计划文件。
    fn write_plan(&self, name: &str) {
        let dir = plans_dir(self.path());
        std::fs::create_dir_all(&dir).expect("create plans dir");
        std::fs::write(dir.join(format!("{name}.md")), "# plan\n").expect("write plan file");
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Build a list from statuses, numbering the tasks.
/// 由状态列表构造一份列表，任务按序号命名。
fn sample(subject: &str, plan: Option<&str>, statuses: &[TodoStatus]) -> TodoList {
    TodoList {
        subject: subject.to_string(),
        create_datetime: "2026-01-01T00:00:00+00:00".to_string(),
        update_datetime: "2026-01-01T00:00:00+00:00".to_string(),
        linked_plan: plan.map(str::to_string),
        todos: statuses
            .iter()
            .enumerate()
            .map(|(index, status)| TodoItem {
                task: format!("task {index}"),
                status: *status,
            })
            .collect(),
    }
}

/// Write a list straight to disk, bypassing the async `save` helper.
/// 直接写盘，绕过异步的 `save`，便于同步测试建立前置数据。
fn write_list(root: &Path, list: &TodoList) {
    let dir = todos_dir(root);
    std::fs::create_dir_all(&dir).expect("create todos dir");
    let text = serde_json::to_string_pretty(list).expect("serialize list");
    std::fs::write(dir.join(format!("{}.json", list.subject)), text).expect("write list file");
}

#[test]
fn status_serializes_as_snake_case() {
    assert_eq!(
        serde_json::to_string(&TodoStatus::InProgress).expect("serialize"),
        "\"in_progress\""
    );
    assert_eq!(
        serde_json::from_str::<TodoStatus>("\"completed\"").expect("deserialize"),
        TodoStatus::Completed
    );
}

#[test]
fn linked_plan_is_omitted_when_absent() {
    let json = serde_json::to_string(&sample("s", None, &[TodoStatus::Pending])).expect("json");
    assert!(!json.contains("linked_plan"));
}

#[test]
fn now_rfc3339_parses_back_into_a_timestamp() {
    let stamp = now_rfc3339();
    assert!(chrono::DateTime::parse_from_rfc3339(&stamp).is_ok());
}

#[test]
fn directories_live_under_the_manualaid_folder() {
    let root = Path::new("/workspace");
    assert_eq!(todos_dir(root), root.join(".ManualAid").join("todos"));
    assert_eq!(plans_dir(root), root.join(".ManualAid").join("plans"));
}

#[test]
fn plan_name_validation_rejects_separators_and_parent_hops() {
    assert!(is_plan_name_valid("plan-a"));
    assert!(is_plan_name_valid("plan_1.2"));
    assert!(!is_plan_name_valid(""));
    assert!(!is_plan_name_valid("   "));
    assert!(!is_plan_name_valid("nested/plan"));
    assert!(!is_plan_name_valid("nested\\plan"));
    assert!(!is_plan_name_valid(".."));
    assert!(!is_plan_name_valid("../escape"));
}

#[test]
fn is_plan_valid_requires_an_existing_plan_file() {
    let root = TempRoot::new("plan-valid");
    assert!(!is_plan_valid(root.path(), "missing"));

    root.write_plan("present");
    assert!(is_plan_valid(root.path(), "present"));
    assert!(!is_plan_valid(root.path(), "nested/present"));
}

#[tokio::test]
async fn load_returns_none_for_a_missing_subject() {
    let root = TempRoot::new("load-missing");
    assert!(load(root.path(), "nope").await.expect("load").is_none());
}

#[tokio::test]
async fn load_reports_malformed_json() {
    let root = TempRoot::new("load-bad");
    let dir = todos_dir(root.path());
    std::fs::create_dir_all(&dir).expect("create dir");
    std::fs::write(dir.join("broken.json"), "{ not json").expect("write");

    assert!(load(root.path(), "broken").await.is_err());
}

#[tokio::test]
async fn save_then_load_round_trips() {
    let root = TempRoot::new("save-round-trip");
    let list = sample("alpha", Some("plan-a"), &[TodoStatus::Pending]);

    let path = save(root.path(), &list).await.expect("save");

    assert_eq!(
        path.file_name().expect("name").to_string_lossy(),
        "alpha.json"
    );
    assert_eq!(load(root.path(), "alpha").await.expect("load"), Some(list));
}

#[test]
fn load_all_without_a_todos_directory_is_empty() {
    let root = TempRoot::new("load-all-absent");
    assert!(load_all(root.path()).is_empty());
}

#[test]
fn load_all_sorts_by_subject_and_skips_malformed_files() {
    let root = TempRoot::new("load-all-sort");
    write_list(root.path(), &sample("bravo", None, &[TodoStatus::Pending]));
    write_list(root.path(), &sample("alpha", None, &[TodoStatus::Pending]));
    std::fs::write(todos_dir(root.path()).join("broken.json"), "{").expect("write");

    let subjects: Vec<String> = load_all(root.path())
        .into_iter()
        .map(|list| list.subject)
        .collect();

    assert_eq!(subjects, vec!["alpha", "bravo"]);
}

#[test]
fn load_all_ignores_non_json_files_and_the_done_subdirectory() {
    let root = TempRoot::new("load-all-scope");
    write_list(root.path(), &sample("active", None, &[TodoStatus::Pending]));

    let dir = todos_dir(root.path());
    std::fs::write(dir.join("legacy.md"), "# legacy").expect("write md");
    let done = dir.join("done");
    std::fs::create_dir_all(&done).expect("create done");
    let archived = serde_json::to_string(&sample("archived", None, &[TodoStatus::Pending]))
        .expect("serialize");
    std::fs::write(done.join("archived.json"), archived).expect("write archived");

    let subjects: Vec<String> = load_all(root.path())
        .into_iter()
        .map(|list| list.subject)
        .collect();

    assert_eq!(subjects, vec!["active"]);
}

#[test]
fn similar_subjects_finds_near_duplicates_only() {
    let root = TempRoot::new("similar-finds");
    write_list(
        root.path(),
        &sample("abcdefghijklmnopqrst", None, &[TodoStatus::Pending]),
    );
    write_list(
        root.path(),
        &sample("abcdefghijklmnopqrsX", None, &[TodoStatus::Pending]),
    );
    write_list(
        root.path(),
        &sample("YYYYYYYYYYYYYYYYYYYY", None, &[TodoStatus::Pending]),
    );

    let found = similar_subjects(root.path(), "abcdefghijklmnopqrsZ");

    assert!(found.contains(&"abcdefghijklmnopqrst".to_string()));
    assert!(found.contains(&"abcdefghijklmnopqrsX".to_string()));
    assert!(!found.contains(&"YYYYYYYYYYYYYYYYYYYY".to_string()));
}

#[test]
fn similar_subjects_excludes_the_subject_itself() {
    let root = TempRoot::new("similar-self");
    write_list(root.path(), &sample("alpha", None, &[TodoStatus::Pending]));

    assert!(similar_subjects(root.path(), "alpha").is_empty());
}

#[test]
fn similar_subjects_with_a_blank_subject_is_empty() {
    let root = TempRoot::new("similar-blank");
    write_list(root.path(), &sample("alpha", None, &[TodoStatus::Pending]));

    assert!(similar_subjects(root.path(), "   ").is_empty());
}

#[test]
fn counts_and_completion_percent_cover_every_status() {
    let list = sample(
        "s",
        None,
        &[
            TodoStatus::Pending,
            TodoStatus::InProgress,
            TodoStatus::InProgress,
            TodoStatus::Completed,
        ],
    );

    assert_eq!(counts(&list), (1, 2, 1));
    assert_eq!(completion_percent(&list), 25);
}

#[test]
fn completion_percent_of_an_empty_list_is_zero() {
    let list = sample("s", None, &[]);

    assert_eq!(counts(&list), (0, 0, 0));
    assert_eq!(completion_percent(&list), 0);
}

#[test]
fn is_complete_requires_at_least_one_task() {
    assert!(!is_complete(&sample("s", None, &[])));
    assert!(is_complete(&sample("s", None, &[TodoStatus::Completed])));
    assert!(!is_complete(&sample(
        "s",
        None,
        &[TodoStatus::Completed, TodoStatus::Pending]
    )));
}

#[test]
fn unfinished_todos_without_candidates_returns_none() {
    let root = TempRoot::new("unfinished-none");
    assert!(unfinished_todos(root.path()).is_none());
}

#[test]
fn unfinished_todos_skips_complete_unlinked_and_orphaned_lists() {
    let root = TempRoot::new("unfinished-skips");
    root.write_plan("plan-a");

    write_list(
        root.path(),
        &sample("complete", Some("plan-a"), &[TodoStatus::Completed]),
    );
    write_list(
        root.path(),
        &sample("unlinked", None, &[TodoStatus::Pending]),
    );
    write_list(
        root.path(),
        &sample("orphaned", Some("missing-plan"), &[TodoStatus::Pending]),
    );
    write_list(
        root.path(),
        &sample("active", Some("plan-a"), &[TodoStatus::Pending]),
    );

    let body = unfinished_todos(root.path()).expect("context");
    let lines: Vec<&str> = body.lines().collect();

    assert_eq!(lines.len(), 2, "only the lead line and one entry");
    assert_eq!(lines[1], "active: 0%");
}

#[test]
fn unfinished_todos_sorts_entries_by_subject() {
    let root = TempRoot::new("unfinished-sort");
    root.write_plan("plan-a");

    write_list(
        root.path(),
        &sample("bravo", Some("plan-a"), &[TodoStatus::Pending]),
    );
    write_list(
        root.path(),
        &sample("alpha", Some("plan-a"), &[TodoStatus::Pending]),
    );

    let body = unfinished_todos(root.path()).expect("context");
    let entries: Vec<&str> = body.lines().skip(1).collect();

    assert_eq!(entries, vec!["alpha: 0%", "bravo: 0%"]);
}

#[test]
fn unfinished_todos_reports_rounded_completion() {
    let root = TempRoot::new("unfinished-percent");
    root.write_plan("plan-a");

    write_list(
        root.path(),
        &sample(
            "alpha",
            Some("plan-a"),
            &[
                TodoStatus::Completed,
                TodoStatus::Completed,
                TodoStatus::InProgress,
            ],
        ),
    );

    let body = unfinished_todos(root.path()).expect("context");

    assert!(body.ends_with("alpha: 66%"));
}

#[tokio::test]
async fn unique_target_appends_index_on_collision() {
    let root = TempRoot::new("unique-target");
    let dir = root.path().join("done");

    let first = unique_target(&dir, "stamp", "subject", "json")
        .await
        .expect("first target");
    std::fs::write(&first, "occupied").expect("occupy first");

    let second = unique_target(&dir, "stamp", "subject", "json")
        .await
        .expect("second target");

    assert_eq!(
        second.file_name().expect("name").to_string_lossy(),
        "stamp-subject-1.json"
    );
}

#[test]
fn archive_stamp_is_filename_safe() {
    assert!(!archive_stamp().contains(':'));
}

#[tokio::test]
async fn archive_moves_plan_and_todo_into_done_directories() {
    let root = TempRoot::new("archive-basic");
    root.write_plan("my-plan");
    let list = sample("alpha", Some("my-plan"), &[TodoStatus::Completed]);
    save(root.path(), &list).await.expect("save");

    let outcome = archive(root.path(), &list).await.expect("archive");

    assert!(!todos_dir(root.path()).join("alpha.json").exists());
    assert!(!plans_dir(root.path()).join("my-plan.md").exists());
    assert!(
        outcome
            .todo_path
            .starts_with(todos_dir(root.path()).join("done"))
    );
    assert_eq!(
        outcome.todo_path.extension().and_then(|ext| ext.to_str()),
        Some("json")
    );
    assert!(
        outcome
            .todo_path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .ends_with("-alpha.json")
    );

    let plan_path = outcome.plan_path.expect("plan archived");
    assert!(plan_path.starts_with(plans_dir(root.path()).join("done")));
    assert!(
        plan_path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .ends_with("-my-plan.md")
    );
}

#[tokio::test]
async fn archive_without_a_linked_plan_moves_only_the_todo() {
    let root = TempRoot::new("archive-no-plan");
    let list = sample("alpha", None, &[TodoStatus::Completed]);
    save(root.path(), &list).await.expect("save");

    let outcome = archive(root.path(), &list).await.expect("archive");

    assert!(outcome.plan_path.is_none());
    assert!(outcome.todo_path.exists());
}

#[tokio::test]
async fn archive_reports_when_only_the_plan_moved() {
    let root = TempRoot::new("archive-partial");
    root.write_plan("my-plan");
    let list = sample("missing", Some("my-plan"), &[TodoStatus::Completed]);
    // The todo document is deliberately never written, so the second move
    // fails after the plan already left `plans/`.
    // 刻意不写 TODO 文档，使第二次移动在计划已离开 `plans/` 之后失败。

    let error = archive(root.path(), &list).await.expect_err("must fail");

    assert!(error.contains("The linked plan was already moved"));
    assert!(!plans_dir(root.path()).join("my-plan.md").exists());
}
