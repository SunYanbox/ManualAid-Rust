# Issue 与 Pull Request 指南

## 标题格式

- Bug：`bug: <简短描述>`（英文）
- 功能请求：`feat(<范围>): <简短描述>`（英文）
- Pull Request：遵循提交规范主题行格式（`<类型>(<范围>): <主题>`）

## 正文双语格式

所有 Issue 和 PR 的正文统一使用以下格式；正文中所有标题统一使用 Markdown 加粗格式（如 `**Summary**`），不得使用 `#`/`##` 标题：

```
[英文正文]

---

<details><summary>中文</summary>
<p>

[中文正文]

</p>
</details>
```

## 项目标签

实际使用的标签（`gh label list`）：

| 标签 | 用途 |
|------|------|
| `bug` | 缺陷 |
| `enhancement` | 新功能/改进 |
| `documentation` | 文档 |
| `good first issue` | 适合新手 |
| `help wanted` | 需要协助 |
| `duplicate` | 重复 |
| `invalid` | 无效 |
| `question` | 需进一步信息 |
| `wontfix` | 不处理 |
| `CI/CD` | CI/CD 变更 |
| `release` | 版本发布 |
| `prompt` | 提示词优化或更改 |
| `refactor` | 代码重构，改善可读性与可维护性，不改变外部行为 |

> `prompt`主要指`crates/i18n/locales/prompts.*.toml`中的更改。

## PR 标签

创建 PR 时，必须根据实际变更从上方标签中选择并添加匹配的 label（使用 `gh pr create --label <标签>` 或创建后 `gh pr edit --add-label <标签>`），不得省略。

## PR 检查

提交 PR 后 CI 会自动运行格式、lint、编译、测试等检查。无需在本地手动运行 `./scripts/ci.*`。
