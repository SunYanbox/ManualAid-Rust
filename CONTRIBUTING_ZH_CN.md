# 贡献指南

感谢您对 ManualAid 的关注！以下指南旨在帮助您顺畅地参与项目开发，同时保持代码库的一致性和质量。

## 开发环境与工具链

本项目统一使用以下工具：

- **Rust 生态**：使用 `cargo` 管理依赖、编译、测试及文档生成。
- **Node.js 生态**（如有前端相关部分）：使用 `pnpm` 管理依赖，请勿使用 npm 或 yarn。

## 开发流程

核心开发流程如下，详细规则见 `docs/issue-pr-guide.md`：

1. **签出新分支**：先 `git fetch` 拉取最新 `origin/main`，再从最新 `main` 签出分支，命名沿用 `<type>/<desc>` 或 `<type>/<issue>-<desc>`（如 `fix/24-...`、`ci/...`）。
2. **提交前检查（每次提交的门槛）**：暂存要提交的文件前，运行 `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项。无需重复执行 `cargo fmt -- --check`。
3. **全量检查由 PR CI 承担**：跨平台的全量检查（格式、lint、文档、测试、覆盖率）由 PR 的 CI（`.github/workflows/ci.yml`）自动运行，本地每次提交**不必**运行全量 `./scripts/ci.*`。
4. **创建 PR 前**：更新 CHANGELOG（规则见 `docs/issue-pr-guide.md`）；尽可能解决基础的测试与覆盖率问题（见「测试要求」）。仅查看覆盖率信息时优先读取缓存 `coverage_with_lines.txt`，代码未更改时不必重跑全量覆盖率测试。
5. **创建 PR 后**：依据 PR CI 结果继续优化测试与覆盖率。
6. **PR 严格按模板编写**：PR 正文**必须**按 `.github/pull_request_template.md` 模板编写，并添加匹配的 label。

## 代码风格与质量

我们遵循 Rust 官方风格指南，并借助工具自动化检查：

- 提交任何 `*.rs` 文件前，**暂存前必跑** `cargo fmt`、`cargo clippy -- -D warnings`、`cargo check` 三项（无需重复 `cargo fmt -- --check`）。
- 确保文档可正常生成：`cargo doc --no-deps`。

## 测试要求

### 测试文件组织

涉及公共 API 的测试，请优先放在 `tests` 目录下对应的集成测试文件中，避免将大量测试实现与源代码混在同一文件内。您可以使用 `metron --per-file crates` 查看各文件的代码、注释、测试占比，辅助评估测试分布。

### 覆盖率目标

我们鼓励保持较高的测试覆盖率。使用 `cargo llvm-cov --show-missing-lines` 检查 `src` 目录的覆盖情况：

- 单个文件的测试覆盖率建议**不低于 85%**，核心模块建议**不低于 95%**。
- 总体覆盖率（Function、Line、Region）建议**不低于 80%**。

部分文件因外部依赖或特殊原因暂不纳入覆盖率要求，已在代码注释中说明（如 `user_dir.rs`、`clipboard.rs`、`init.rs`）。

### stdout 测试处理

对于向标准输出打印内容的测试，请将输出重定向到内部字符串变量后再进行断言，避免污染终端输出。同时确保方案在沙箱和真实终端下均能正常工作。

## 依赖管理

新增 crate 或 feature 时，请使用 `cargo add xxx` 命令添加，**不指定版本号**，以始终采用最新兼容版本。

## 提交与 Pull Request

- 提交信息请遵循[约定式提交规范](https://www.conventionalcommits.org/en/v1.0.0/)（如 `feat:`, `fix:`, `docs:` 等）。详细指南见 `docs/commit-conventions.md`。
- **PR 正文必须按 `.github/pull_request_template.md` 模板编写**，并添加匹配的 label；模板说明见 `docs/issue-pr-guide.md`。
- 创建 PR 前更新 CHANGELOG（规则见 `docs/issue-pr-guide.md`）；纯文档等无需条目时除外。在 PR 中，请确保所有检查（格式、lint、编译、文档、测试）均已通过。

> **提示**：跨平台的全量检查（格式、lint、编译、文档、测试、覆盖率）由 PR 的 CI（`.github/workflows/ci.yml`）自动运行。交付前可按需使用 `./scripts/ci.*`（根据平台选择合适脚本）一键运行全量检查；提交时仅需在暂存前跑 `cargo fmt` + `cargo clippy -- -D warnings` + `cargo check`。

## 注释风格

注释重在解释“为什么”而非“做什么”。欢迎使用注释澄清复杂逻辑、标注待办事项（`TODO`/`FIXME`）、提示副作用或外部依赖。同时：

- 避免重复代码本身的废话注释。
- 避免过时、误导或“日记式”注释（如署名、日期）。
- 不要用注释保留大段废弃代码，应依赖版本控制历史。
- **禁止使用装饰性分隔线**（如 `----`、`====` 或 Unicode 装饰线）。

文档注释（`//!` 和 `///`）建议采用双语（中英文）简洁描述，详细说明可使用 `# Description` / `# 描述` 结构。
