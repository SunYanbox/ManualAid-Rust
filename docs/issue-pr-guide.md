# Issue 与 Pull Request 指南

## 开发流程

核心开发流程如下，与文档约定（分支命名、CHANGELOG、PR 规范）配合使用：

1. **签出新分支**：先 `git fetch` 拉取最新 `origin/main`，再从最新 `main` 签出分支，命名沿用 `<type>/<desc>` 或 `<type>/<issue>-<desc>`（如 `fix/24-...`、`ci/...`）。
2. **提交前检查（每次提交的门槛）**：暂存要提交的文件前，运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项；无需重复 `cargo fmt -- --check`，也无需在本地每次跑全量 CI。
3. **全量检查由 PR CI 承担**：跨平台全量检查（格式、lint、编译、文档、测试、覆盖率）由 PR 的 CI（`.github/workflows/ci.yml`）自动运行。
4. **创建 PR 前**：更新 CHANGELOG（见「CHANGELOG 更新」）；尽可能解决基础的测试与覆盖率问题（代码已改变则重跑全量覆盖率更新 `coverage_with_lines.txt`；仅查看信息时读缓存）。
5. **创建 PR 后**：依据 PR CI 结果继续优化测试与覆盖率。
6. **PR 严格按模板编写**：正文**必须**按 `.github/pull_request_template.md` 模板编写（见下），并添加匹配的 label。

## 标题格式

- Bug：`bug: <简短描述>`（英文）
- 功能请求：`feat(<范围>): <简短描述>`（英文）
- Pull Request：遵循提交规范主题行格式（`<类型>(<范围>): <主题>`）

## 正文双语格式

所有 Issue 和 PR 的正文统一使用以下格式；正文中所有标题统一使用 Markdown 加粗格式（如 `**Summary**`），不得使用 `#`/`##` 标题。

**创建 PR 时，正文必须严格按 `.github/pull_request_template.md` 模板文件编写**，完整保留其空行与 `<details>` 折叠结构（下为与之一致的模板，实际以该文件为准）：

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

## CHANGELOG 更新

交付/推送 PR 前，核对两个 `CHANGELOG*.md` 是否需要更新，并按面向对象写入对应 `[Unreleased]` 段：

- **根目录 `CHANGELOG_ZH_CN.md`（面向开发者）**：记录开发者应当了解的变更——除用户可见变更外，还包括公有 API 变更、API 行为变更、破坏性变更等；仅文档更新、格式化代码、补充覆盖率测试等无开发者应知信息的变更，可不新增条目。
- **`docs/changelog/CHANGELOG_ZH_CN.md`（面向用户）**：只记录用户可见变更——UI 变化、错误修复、新功能、功能变更、工具结果格式/输出变更、工具优化、新增配置功能、提示词优化等；仓库根目录与 docs 目录的文档变更、API 相关变更等用户无需了解的内容，不写入本文件。

> `docs/changelog/CHANGELOG_ZH_CN.md` 被内置更新日志查看器编译嵌入并解析（`crates/i18n`），必须保持 `## [版本]` / `## [Unreleased]` 块结构。

**维护规则**：
- `[Unreleased]` 标题必须始终保留，不可删除。
- 日常新增内容时，直接在 `[Unreleased]` 下添加对应子标题（`### 新增`、`### 变更`、`### 修复` 等）。
- 发布新版本时，将 `[Unreleased]` 下所有内容移至新版本标题（如 `## [x.y.z] - yyyy-mm-dd`）下，然后保留一个空白的 `[Unreleased]` 标题供后续使用。

## PR 检查

CI（`.github/workflows/ci.yml`）在变更涉及 `crates/**`、`Cargo.toml`、`Cargo.lock`、`.github/workflows/**`、`codecov.yml` 时自动运行格式、lint、编译、文档与测试等检查；无代码变更时无需在本地手动运行 `./scripts/ci.*`。

**创建 PR 后**：查看 PR CI 结果并据此继续优化——补充测试以提高覆盖率（新增/修改代码后重跑全量覆盖率并更新 `coverage_with_lines.txt`，仅查看信息时读缓存），修复 CI 报出的格式/lint/测试问题，直至检查通过。

**CHANGELOG 自动提醒**：`.github/workflows/changelog-reminder.yml` 会在 PR 变更了 `crates/**`、`Cargo.toml`、`Cargo.lock` 或 `.github/**` 但未更新 `CHANGELOG*.md` 时，以 sticky 评论自动提醒补充。若变更无用户/开发者影响，可忽略该提醒。

纯文档等非代码路径的 PR 不会触发上述 CI，交付前应按变更类型做针对性验证（如 markdown 链接与格式核对，或按需运行 `./scripts/ci.*`）。
