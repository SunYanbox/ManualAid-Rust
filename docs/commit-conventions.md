# 提交规范

遵循[约定式提交规范](https://www.conventionalcommits.org/zh-hans/v1.0.0/)。

## 项目特定要求

1. **提交消息必须用英文编写**。
2. **提交消息必须客观**：仅根据本次 diff 描述变更内容，不推断意图，不引入与 diff 无关的上下文。
3. **提交前检查**：暂存要提交的文件前，必须运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项本地检查（此为每次提交的门槛；跨平台全量检查由 PR CI 承担）。详细流程见 `docs/issue-pr-guide.md` 的「开发流程」章节。

PR 合并时会自动在提交消息末尾追加 PR 序号（如 `(#43)`），无需手动添加。
