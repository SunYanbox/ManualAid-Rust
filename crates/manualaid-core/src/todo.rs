//! Persistent TODO lists: schema, statistics, prompt context and archiving.
//! 持久化 TODO 列表：结构、统计、提示词上下文与归档。
//!
//! A list lives at `<root>/.ManualAid/todos/<subject>.json` and may bind to a
//! plan file under `<root>/.ManualAid/plans/`. Once every task is complete the
//! pair is moved into the matching `done/` directory; that move deliberately
//! bypasses the audit queue because it is bookkeeping rather than user content.
//! 列表存放于 `<root>/.ManualAid/todos/<subject>.json`，并可绑定
//! `<root>/.ManualAid/plans/` 下的计划文件。全部任务完成时，两者被移入各自
//! 的 `done/` 目录；该移动刻意绕过审计队列，因为它属于记账而非用户内容。
//!
//! Only `todos/*.json` is read back: the legacy `*.md` files kept by the
//! previous prompt-only workflow are left untouched so both schemes can
//! coexist during the migration.
//! 只回读 `todos/*.json`：此前仅靠提示词驱动的工作流留下的 `*.md` 文件保持
//! 原样，使两套方案在迁移期并存。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::async_fs;
use crate::error::{CoreError, CoreResult};

/// Similarity above which two subjects are reported as near-duplicates.
/// 两个主题被视为近似重复的相似度阈值。
pub const SIMILAR_SUBJECT_RATIO: f32 = 0.90;

/// Lifecycle state of a single TODO entry.
/// 单条 TODO 的生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Not started yet.
    /// 尚未开始。
    Pending,
    /// Currently being worked on.
    /// 正在进行。
    InProgress,
    /// Finished.
    /// 已完成。
    Completed,
}

/// One TODO entry: the task text plus its state.
/// 一条 TODO：任务文本与其状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    /// Human-readable task description.
    /// 人类可读的任务描述。
    pub task: String,
    /// Current state of this task.
    /// 该任务的当前状态。
    pub status: TodoStatus,
}

/// A full TODO list, persisted as one JSON document.
/// 一份完整 TODO 列表，持久化为单个 JSON 文档。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoList {
    /// Unique subject; also the file stem under `todos/`.
    /// 唯一主题；同时是 `todos/` 下的文件名主干。
    pub subject: String,
    /// RFC3339 creation stamp, written once and preserved afterwards.
    /// RFC3339 创建时间，写入一次后保持不变。
    pub create_datetime: String,
    /// RFC3339 stamp refreshed on every write.
    /// 每次写入都会刷新的 RFC3339 时间。
    pub update_datetime: String,
    /// Optional plan file stem under `plans/`.
    /// 可选的 `plans/` 下计划文件名主干。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_plan: Option<String>,
    /// Every task in the list; the tool always submits the full set.
    /// 列表中的所有任务；工具每次提交完整集合。
    pub todos: Vec<TodoItem>,
}

/// Result of moving a completed list (and its plan) into `done/`.
/// 将已完成的列表（及其计划）移入 `done/` 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveOutcome {
    /// Final location of the archived TODO JSON.
    /// 归档后的 TODO JSON 最终位置。
    pub todo_path: PathBuf,
    /// Final location of the archived plan file, when one was linked.
    /// 归档后的计划文件最终位置（存在绑定时）。
    pub plan_path: Option<PathBuf>,
}

/// Current local time as an RFC3339 stamp with second precision.
/// 当前本地时间的 RFC3339 时间戳，精确到秒。
pub fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// Directory holding every TODO list.
/// 存放所有 TODO 列表的目录。
pub fn todos_dir(root: &Path) -> PathBuf {
    root.join(".ManualAid").join("todos")
}

/// Directory holding every plan file a list may link to.
/// 存放列表可绑定的全部计划文件的目录。
pub fn plans_dir(root: &Path) -> PathBuf {
    root.join(".ManualAid").join("plans")
}

/// Path of one TODO list document.
/// 单个 TODO 列表文档的路径。
fn todo_path(root: &Path, subject: &str) -> PathBuf {
    todos_dir(root).join(format!("{subject}.json"))
}

/// Whether `name` is usable as a plan file stem: single path segment, no
/// parent-directory hop. Rejecting separators here is what keeps the tool from
/// reaching outside `plans/` even though no parameter carries path semantics.
/// `name` 是否可作为计划文件名主干：单个路径段，不含上级目录跳转。在此拒绝
/// 分隔符，是工具即使在参数不带路径语义时也不会写到 `plans/` 之外的原因。
pub fn is_plan_name_valid(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && !trimmed.contains("..")
}

/// Whether the linked plan file currently exists under `plans/`.
///
/// A plan moved into `plans/done/` is intentionally *not* found here: callers
/// treat that as "already finished" and stop listing the subject, without
/// re-triggering an archive.
/// 绑定的计划文件当前是否存在于 `plans/` 下。
///
/// 已移入 `plans/done/` 的计划刻意不在此命中：调用方据此视为“已完成”并停止
/// 列出该主题，而不会再次触发归档。
pub fn is_plan_valid(root: &Path, name: &str) -> bool {
    is_plan_name_valid(name)
        && plans_dir(root)
            .join(format!("{}.md", name.trim()))
            .is_file()
}

/// Load one TODO list by subject, or `None` when it does not exist yet.
/// 按主题加载单个 TODO 列表；尚不存在时返回 `None`。
pub async fn load(root: &Path, subject: &str) -> CoreResult<Option<TodoList>> {
    let path = todo_path(root, subject);
    // `async_fs::read_file` flattens every I/O failure into `CoreError::Io`,
    // which would hide the "not created yet" case that callers rely on. Go
    // through `tokio::fs` here so `ErrorKind::NotFound` stays observable.
    // `async_fs::read_file` 会把所有 I/O 失败压平成 `CoreError::Io`，从而掩盖
    // 调用方依赖的“尚未创建”情形。此处直接走 `tokio::fs`，使
    // `ErrorKind::NotFound` 仍可辨别。
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| CoreError::Parse(format!("invalid todo JSON `{}`: {e}", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(CoreError::Io(format!(
            "cannot read file `{}`: {error}",
            path.display()
        ))),
    }
}

/// Load every TODO list, sorted by subject.
///
/// Unreadable or malformed documents are skipped rather than surfaced: the
/// list feeds the prompt context, where one broken file must not hide the
/// rest. The scan is deliberately flat so `todos/done/` stays out of scope.
/// 加载所有 TODO 列表，按主题排序。
///
/// 无法读取或格式错误的文档会被跳过而非上报：该列表用于提示词上下文，单个损坏
/// 文件不应掩盖其余内容。扫描刻意保持扁平，使 `todos/done/` 不在范围内。
pub fn load_all(root: &Path) -> Vec<TodoList> {
    let dir = todos_dir(root);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut lists: Vec<TodoList> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|text| serde_json::from_str::<TodoList>(&text).ok())
        .collect();
    lists.sort_by(|a, b| a.subject.cmp(&b.subject));
    lists
}

/// Persist one TODO list, creating the `todos/` directory when missing.
///
/// Timestamps are the caller's responsibility: [`now_rfc3339`] builds them and
/// the tool layer decides which of the two to refresh.
/// 持久化单个 TODO 列表，`todos/` 目录缺失时自动创建。
///
/// 时间戳由调用方负责：[`now_rfc3339`] 负责生成，工具层决定刷新哪一个。
pub async fn save(root: &Path, list: &TodoList) -> CoreResult<PathBuf> {
    let path = todo_path(root, &list.subject);
    let text = serde_json::to_string_pretty(list)
        .map_err(|e| CoreError::Other(format!("cannot serialize todo list: {e}")))?;
    async_fs::write_file(&path, text).await?;
    Ok(path)
}

/// Existing subjects whose similarity to `subject` reaches
/// [`SIMILAR_SUBJECT_RATIO`], used to suggest reusing a list instead of
/// creating a near-duplicate one.
/// 与 `subject` 相似度达到 [`SIMILAR_SUBJECT_RATIO`] 的既有主题，用于建议复用
/// 列表而不是新建近似重复的一份。
pub fn similar_subjects(root: &Path, subject: &str) -> Vec<String> {
    let trimmed = subject.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    load_all(root)
        .into_iter()
        .filter(|list| list.subject != trimmed)
        .filter(|list| {
            TextDiff::from_chars(trimmed, &list.subject).ratio() >= SIMILAR_SUBJECT_RATIO
        })
        .map(|list| list.subject)
        .collect()
}

/// Count `(pending, in_progress, completed)` entries in a list.
/// 统计列表中的 `(待办, 进行中, 已完成)` 数量。
pub fn counts(list: &TodoList) -> (usize, usize, usize) {
    let mut pending = 0usize;
    let mut in_progress = 0usize;
    let mut completed = 0usize;
    for item in &list.todos {
        match item.status {
            TodoStatus::Pending => pending += 1,
            TodoStatus::InProgress => in_progress += 1,
            TodoStatus::Completed => completed += 1,
        }
    }
    (pending, in_progress, completed)
}

/// Share of completed tasks, rounded down to a whole percent.
/// 已完成任务所占比例，向下取整为整数百分比。
pub fn completion_percent(list: &TodoList) -> u8 {
    if list.todos.is_empty() {
        return 0;
    }
    let (_, _, completed) = counts(list);
    ((completed * 100) / list.todos.len()) as u8
}

/// Whether every task in the list is complete. An empty list is not complete.
/// 列表中的任务是否全部完成。空列表不算完成。
pub fn is_complete(list: &TodoList) -> bool {
    !list.todos.is_empty()
        && list
            .todos
            .iter()
            .all(|item| item.status == TodoStatus::Completed)
}

/// Render the unfinished TODO context shared by prompt injection and the
/// `/todos` inline command.
///
/// Returns `None` when nothing qualifies. A list is skipped when it is already
/// complete, when it has no linked plan, or when that plan is no longer
/// present under `plans/` (the state left behind after an earlier archive).
/// The returned text carries no outer tag: callers wrap it themselves.
/// 渲染未完成 TODO 上下文，供提示词注入与 `/todos` 内联命令共用。
///
/// 没有符合项时返回 `None`。已完成的列表、没有绑定计划的列表，以及绑定计划已
/// 不在 `plans/` 下的列表（此前归档后留下的状态）都会被跳过。返回文本不含外层
/// 标签，由调用方自行包裹。
pub fn unfinished_todos(root: &Path) -> Option<String> {
    let entries: Vec<String> = load_all(root)
        .into_iter()
        .filter(|list| !is_complete(list))
        .filter(|list| match &list.linked_plan {
            Some(plan) => is_plan_valid(root, plan),
            None => false,
        })
        .map(|list| format!("{}: {}%", list.subject, completion_percent(&list)))
        .collect();

    if entries.is_empty() {
        return None;
    }

    let mut body = i18n::t_str("prompt.system.todo-context-lead");
    for entry in entries {
        body.push('\n');
        body.push_str(&entry);
    }
    Some(body)
}

/// Move a completed list and its plan into the matching `done/` directories.
///
/// The plan moves first so a failure leaves the TODO list in place, where it is
/// still discoverable. When the plan move succeeds but the TODO move fails the
/// error says so explicitly, since the caller cannot undo the first move.
/// 将已完成的列表及其计划移入各自的 `done/` 目录。
///
/// 先移动计划，失败时 TODO 列表仍在原位、仍可被发现。若计划已移动而 TODO 移动
/// 失败，错误信息会明确说明，因为调用方无法撤销前一次移动。
pub async fn archive(root: &Path, list: &TodoList) -> Result<ArchiveOutcome, String> {
    let stamp = archive_stamp();
    let mut plan_path = None;

    if let Some(plan) = &list.linked_plan {
        let plan = plan.trim();
        let source = plans_dir(root).join(format!("{plan}.md"));
        let target = unique_target(&plans_dir(root).join("done"), &stamp, plan, "md").await?;
        tokio::fs::rename(&source, &target).await.map_err(|e| {
            format!(
                "cannot archive plan `{}` to `{}`: {e}",
                source.display(),
                target.display()
            )
        })?;
        plan_path = Some(target);
    }

    let source = todo_path(root, &list.subject);
    let target =
        unique_target(&todos_dir(root).join("done"), &stamp, &list.subject, "json").await?;
    tokio::fs::rename(&source, &target).await.map_err(|e| {
        let moved_plan = match &plan_path {
            Some(path) => format!(
                " The linked plan was already moved to `{}`.",
                path.display()
            ),
            None => String::new(),
        };
        format!(
            "cannot archive todo `{}` to `{}`: {e}.{moved_plan}",
            source.display(),
            target.display()
        )
    })?;

    Ok(ArchiveOutcome {
        todo_path: target,
        plan_path,
    })
}

/// RFC3339 stamp safe for file names: `:` is illegal on Windows.
/// 可用于文件名的 RFC3339 时间戳：`:` 在 Windows 上非法。
fn archive_stamp() -> String {
    now_rfc3339().replace(':', "-")
}

/// Pick `<dir>/<stamp>-<subject>.<ext>`, appending `-1`, `-2`, … on collision.
/// 选择 `<dir>/<stamp>-<subject>.<ext>`，冲突时追加 `-1`、`-2`……
async fn unique_target(
    dir: &Path,
    stamp: &str,
    subject: &str,
    ext: &str,
) -> Result<PathBuf, String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("cannot create `{}`: {e}", dir.display()))?;

    let base = format!("{stamp}-{subject}");
    let first = dir.join(format!("{base}.{ext}"));
    if !first.exists() {
        return Ok(first);
    }

    for index in 1u32.. {
        let candidate = dir.join(format!("{base}-{index}.{ext}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!("the collision loop only exits by returning a free name")
}

#[cfg(test)]
#[path = "todo_tests.rs"]
mod tests;
