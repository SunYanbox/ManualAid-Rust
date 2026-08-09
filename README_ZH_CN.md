# ManualAid

中文 / [English](README.md)

![Rust](https://img.shields.io/badge/rust-1.97.0+-blue.svg)
![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)

一款**纯本地、需人工介入**的复制粘贴助手，专为 LLM 工作流设计。

> **版本**: 0.1.0 | **Rust**: >=1.97.0

> 当前项目为对原`ManualAid(Pythhon版)`的基于Rust的重构版本。

### 约束

- 提交任何`*.rs`文件前，**必须**通过以下所有检查：
  - `cargo fmt -- --check` — 检查代码风格
  - `cargo clippy -- -D warnings` — 捕获常见错误和 lint 违规
  - `cargo check` — 验证编译
  - `cargo llvm-cov` — 运行代码覆盖率分析，输出结果中 **TOTAL** 行的 Function、Line、Region 三项覆盖率均需**≥80%**，除非相关文件由于各种原因难以提升覆盖率。
- 请遵循[Rust 官方风格指南](https://doc.rust-lang.org/stable/style-guide/index.html)和[约定式提交规范](https://www.conventionalcommits.org/en/v1.0.0/)。

## 法律免责声明

ManualAid 是一款**纯本地、需人工介入的辅助工具**。它旨在辅助手动复制粘贴工作流，**不支持**与任何LLM平台的自动化交互。

**用户须自行负责:**

1. 遵守所使用LLM平台的服务条款(ToS)。
2. 确保使用方式不违反频率限制、自动化禁令或其他政策。

**ManualAid的作者明确声明不承担以下责任:**

- 滥用本工具进行自动化请求、绕过付费墙或滥用LLM服务。
- 因上述滥用行为导致的任何账户封禁、法律诉讼或损害。

如果您 Fork 本项目，必须保留此免责声明，并确保您的修改不会促进或启用自动化滥用。
