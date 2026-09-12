## 开发流程

核心开发流程如下，提交与 PR 相关细节见本文件后续章节与 `docs/issue-pr-guide.md`。

1. **签出新分支**：从最新 `main` 签出分支（先 `git fetch` 拉取最新 `origin/main`）。分支命名沿用 `<type>/<short-desc>` 或 `<type>/<issue>-<desc>` 格式（如 `fix/24-...`、`ci/...`）。
2. **提交前检查**：暂存（stage）要提交的文件前运行三项本地检查：`cargo fmt`、`cargo clippy -- -D warnings`、`cargo check`。此即每次提交的门槛，无需重复 `fmt -- --check`，也无需在本地每次跑全量 CI。
3. **全量检查由 PR CI 承担**：跨平台的全量检查（fmt/clippy/doc/test/coverage）由 PR 的 CI（`.github/workflows/ci.yml`）自动运行，本地**不必**每次提交都跑全量 `./scripts/ci.*`。
4. **创建 PR 前**：更新 CHANGELOG（规则见「CHANGELOG 维护规则」）；尽可能解决基础的测试与覆盖率问题（详见「测试与交付检查」）。
5. **创建 PR 后**：依据 PR CI 结果继续优化测试与覆盖率。
6. **PR 严格按模板编写**：PR 正文**必须**按 `.github/pull_request_template.md` 模板编写（见「PR / Issue 规范」），并添加匹配的 label。

## 包管理与构建工具

本项目统一使用以下工具进行依赖管理和项目构建：

- **Rust 生态**：使用 Cargo（即 `cargo` 命令）管理依赖、编译、测试及文档生成。
- **Node.js 生态**：使用 pnpm（即 `pnpm` 命令）管理前端相关依赖，禁止使用 npm 或 yarn。
- **依赖添加**：新增 crate 或 feature 通过 `cargo add xxx` 添加且不指定版本号。多 crate 共同依赖推荐提取至 `[workspace.dependencies]`。

## 提交与 Issue/PR 规范

**生成提交消息或 PR/Issue 内容前，先查看需要提交/需要推送的那些文件相对上一提交或目标分支的实际变更，依据 diff 编写，严禁根据文件列表或对话历史推断。遵循约定式提交规范，使用英文、客观描述变更。非纯格式化变更补充正文说明背景与范围。**

### 提交规范

提交规范遵循约定式提交，使用英文编写。

详细规范见 `docs/commit-conventions.md`。

### PR / Issue 规范

**格式要求与模板**：
- **标题**：Bug 用 `bug: <简短描述>`，功能用 `feat(<范围>): <简短描述>`。
- **正文双语**：英文在前，分隔线后折叠中文。正文内所有标题使用 Markdown 加粗（如 `**Summary**`），不使用 `#`/`##`。
- **PR 标签**：依据变更添加匹配 label（如 `bug`, `enhancement`, `documentation`, `refactor`, `prompt`）。
- **Issue语境**：Issue语境保持过去时，只描述问题发生的背景、发生时的情况等信息。

创建/编辑 PR 时，正文**必须**按 `.github/pull_request_template.md` 模板文件编写，完整保留其 `**Summary**`、`---`、`<details><summary>中文</summary>` 等空行与双语结构；下为与之一致的模板摘要：

```markdown
[英文正文]

---

<details><summary>中文</summary>
<p>

[中文正文]

</p>
</details>
```

详细提交规范见 `docs/commit-conventions.md`；标题与正文模板、标签列表及 PR 检查说明见 `docs/issue-pr-guide.md`。

交付 PR 前，核对根目录与 `docs/changelog/` 下的 `CHANGELOG*.md` 是否需要更新；更新范围与写入规则见 `docs/issue-pr-guide.md` 的「CHANGELOG 更新」章节。

**CHANGELOG 维护规则**：
- `[Unreleased]` 标题必须始终保留在 CHANGELOG 文件中，不可删除。
- 日常新增内容（功能、变更、修复等）时，直接在 `[Unreleased]` 标题下添加对应的子标题（`### 新增`、`### 变更`、`### 修复` 等），无需等待发布。
- 发布新版本时，将 `[Unreleased]` 下的所有内容移至新版本标题（如 `## [x.y.z] - yyyy-mm-dd`）下，然后保留一个空白的 `[Unreleased]` 标题（不含任何子标题），供后续开发使用。

**版本更新规则**：
- 各 crate 一般情况下按语义化版本升级：破坏性变更/新增功能升级 minor，修复升级 patch；0.x 阶段的 minor 视为包含破坏性变更。
- `i18n` 仅译文（locales）更改视为 patch 升级。
- `manualaid-cli` 每次发布时必须升级版本，升级幅度视其对外行为变化而定：新增对外能力升级 minor，修复对外行为升级 patch。
- 升级后同步更新根 `Cargo.toml` 的 `[workspace.dependencies]` 中对应 crate 版本。
- **版本号仅在发布（Release）时更新**：日常开发提交（功能、修复、重构等）不修改任何 crate 的版本号；只有准备发布新版本时才按上述语义化规则统一升级版本并同步 `[workspace.dependencies]`。

## 注释风格

**核心原则**：解释“为什么”，而非“做什么”。

**禁止**：
- 重复代码本身的废话
- 过时或误导的注释
- 注释掉的大段废弃代码
- “日记式”注释（署名、日期）
- 装饰性分隔线（`----`、`====`、`── ──` 等）

**文档注释格式**：
- 模块级（`//!`）和函数/结构体（`///`）均需双语（英文在前，中文在后）

详细说明见 `docs/comment-style.md`。

## 双语同步与翻译规范

项目中所有成对存在的中英文件（`crates/i18n/locales/*.toml`、`CHANGELOG*.md`、`README*.md`、`docs/zh-cn/` 等）必须遵守：

- **严格同构**：两侧的条目数量、结构层次与信息量必须完全一致，不得任何一侧多出或缺失内容。修改一侧时必须同步核对另一侧，不得只改单侧。
- **意译大于直译**：按目标语言的表达习惯重写，而非逐词对应。直译产生的生硬表述与歧义句式应予改写；同一含义在两种语言中可以使用完全不同的句式。面向 Agent 的提示词尤其应描述 Agent 可观察的事实（如“工具结果的 `[TOOL_RESULT ...]` 头部带 `success=false`”），而非系统内部实现（如“解析失败”）。
- **保留专有名词**：标签名（`<rules>`、`<path-rules>`）、标识符（`read`、`func_calls`）、配置项名与 API 名称一律原文照录，不翻译、不意译、不额外解释。

## 测试与交付检查

- **组织方式**：公共 API 测试置于 `tests/` 目录；私有或 `pub(crate)` 测试置于源码附近的 `_tests.rs` 或子模块的 `tests/` 目录，避免实现与测试混杂。使用 `metron --per-file crates` 检查占比。
- **关键规则**：
  - `i18n!()` 仅在 `i18n` 库 crate 内调用一次；二进制与其余 crate 统一通过 `i18n::t_str()` 取翻译。
  - 实现较长或测试需按主题拆分时，采用同名 `.rs` + 同名子目录：文件末尾用 `#[cfg(test)] mod tests;` 指向 `<name>/tests/`；实现仍是单文件时，用 `#[cfg(test)] #[path = "<source_name>_tests.rs"] mod tests;`，测试文件开头 `use super::*;`。默认优先少建目录，测试主题多、文件长时再拆目录。
  - 各 crate 集成测试目录与 `src` 模块一一对应：`manualaid-core/tests/`、`manualaid-cli/tests/`（`api.rs`、`commands.rs` 以 `#[path]` 聚合对应子目录）、`manualaid-ws/tests/`（`config.rs`、`context.rs`、`prompt.rs`）、`i18n/tests/`。

- **提交前本地检查（每次提交的门槛）**：暂存要提交的文件前，运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项（见「开发流程」）。跨平台全量检查由 PR 的 CI（`.github/workflows/ci.yml`）承担，本地每次提交**不必**运行全量 `./scripts/ci.*`。
- **交付/PR 前全量检查**：交付前运行`./scripts/ci.*`(根据平台选择合适的)做全量检查，其中包含：`cargo clippy -- -D warnings`、`cargo fmt -- --check`、`cargo llvm-cov -q --show-missing-lines > coverage_with_lines.txt`等。
- 单个文件的测试代码覆盖率应尽可能**不低于85%**，其中核心模块的覆盖率应尽可能**不低于95%**；总体覆盖率（Function、Line、Region 三项）均需**不低于80%**。
- 全量覆盖率测试（`cargo llvm-cov`，同时运行测试并统计覆盖）优先于仅运行全量测试。
- 覆盖率结果会被缓存到`coverage_with_lines.txt`；仅查看覆盖率信息（如 TOTAL 行）时优先读取缓存，代码未更改时**不要**重跑全量覆盖率测试。

### 外部有状态资源隔离

**stdout 重定向**：所有涉及向 `stdout` 输出内容的测试场景，均应改为将输出重定向至程序内部的字符串变量，再对该变量进行验证，以避免终端输出受到污染；同时需确保该方案在沙箱环境和真实终端下均能正常工作。

**剪贴板测试**：涉及剪贴板的测试遵循以下原则，以避免污染用户系统剪贴板，并保证测试在无头 CI 与 Windows 环境中稳定：
- 所有需要读写剪贴板的行为，优先使用 `MockClipboard` 进行单元测试，覆盖写入成功、写入失败、读取成功、读取失败等分支。
- 依赖真实系统剪贴板才能覆盖的场景（如外部进程实际读取系统剪贴板），可以不测试、放弃该测试用例，或仅测试到“调用真实剪贴板”的入口而不验证真实剪贴板内容。
- 禁止在单元测试或集成测试中直接操作真实系统剪贴板并对其内容做断言；需要真实剪贴板的集成/流程测试，应改为断言程序输出的成功/失败提示，而非轮询系统剪贴板。
- 若确需测试真实剪贴板场景，应将测试标记为 `ignore` 或改为手工测试，避免默认 CI 运行失败并污染用户剪贴板。

stdout 重定向与剪贴板测试同理：通过 mock 或输出断言隔离外部有状态资源。

### 覆盖率豁免文件

以下文件暂不纳入覆盖率提升要求：
- `crates/manualaid-core/src/user_dir.rs`：其错误分支依赖操作系统状态，Windows 下 `dirs` 会调用 KnownFolder API，测试进程内无法模拟 `None` 场景。
- `crates/i18n/src/init.rs`：仅包含一行编译期宏调用，不存在可执行的运行时逻辑。
- `crates/manualaid-cli/src/commands/copy.rs`：`run_copy` 公开入口唯一使用真实系统剪贴板，按剪贴板隔离规范不应在测试中直接操作真实剪贴板；可注入 provider 的核心分发路径已由单元测试覆盖。

### 测试编写规范

编写测试时，必须遵循以下“好的测试”原则，并明确避免下述“不好的测试”。

**好的测试：** 命名描述业务行为、遵循 AAA（Arrange-Act-Assert）模式、结果确定、相互独立、测行为而非实现、一个测试一个行为、够快、失败信息可定位、只在边界用 mock、覆盖边界值。

**不好的测试（应避免）：** 随机失败（flaky）、耦合实现细节、共享状态依赖、无断言或吞掉异常、巨型测试、魔法数字、测试琐碎内容、依赖真实外部环境、僵尸测试。

## 开发状态

我们处于快速开发阶段，在实现新功能时只考虑数据相关兼容性，不需要保留不用的结构体属性；配置文件保持向前兼容。
