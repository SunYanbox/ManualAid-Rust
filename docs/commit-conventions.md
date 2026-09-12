# 提交规范

遵循[约定式提交规范](https://www.conventionalcommits.org/zh-hans/v1.0.0/)。

## 项目特定要求

1. **提交消息必须用英文编写**。
2. **提交消息必须客观**：仅根据本次 diff 描述变更内容，不推断意图，不引入与 diff 无关的上下文。
3. **提交前检查**：当本次提交包含代码或 `i18n` crate 的译文（locales）更改时，暂存要提交的文件前须运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项本地检查；纯文档变更无需运行。跨平台全量检查由 PR CI 承担，详细流程见 `AGENTS.md` 的「开发流程」章节。

PR 合并时会自动在提交消息末尾追加 PR 序号（如 `(#43)`），无需手动添加。
