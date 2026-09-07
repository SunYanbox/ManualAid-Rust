# ManualAid

中文 / [English](README.md)

![Rust](https://img.shields.io/badge/rust-1.97.0+-blue.svg)
![GitHub top language](https://img.shields.io/github/languages/top/SunYanbox/ManualAid-Rust)
![GitHub License](https://img.shields.io/github/license/SunYanbox/ManualAid-Rust)
![Codecov](https://img.shields.io/codecov/c/github/SunYanbox/ManualAid-Rust)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SunYanbox/ManualAid-Rust/.github%2Fworkflows%2Fci.yml)
[![Changelog](https://img.shields.io/badge/Changelog-View-blue)](CHANGELOG_ZH_CN.md)

一款**纯本地、需人工介入**的复制粘贴助手，专为 LLM 工作流设计，基于提示词工程驱动。

> 当前项目为对原 `ManualAid（Python 版）` 的基于 Rust 的重构版本。

## 功能特性

- **基于提示词工程的交互式 Agent Loop** — 无参数运行 `manualaid-cli` 进入交互式会话。系统基于提示词工程原则构建上下文感知提示词（工作区布局、git 状态、已启用工具、已加载技能），你将其粘贴到 LLM 聊天中，然后将 LLM 返回的工具调用响应粘贴回来。ManualAid 解析、审计、执行工具，并将结果返回用于下一轮对话。
- **工具系统** — 内置工具：`Read`、`Edit`、`Write`、`Shell` 和 `Skill`。读取操作即时执行；编辑/写入操作默认需要用户明确审批（`manual` 模式），工作区内的变更可使用 `accept-edit` 模式自动放行。
- **技能系统** — 技能定义为 `SKILL.md` 文件（YAML frontmatter + Markdown 正文），从项目与全局 agent 目录（`.claude/`、`.ManualAid/` 等）发现。仅技能名称与描述加载到系统提示词中；完整指令仅在 Agent 调用 Skill 工具时注入。
- **会话日志与监控** — 每轮工具调用均记录执行耗时与 Token 估算；可通过内置命令查看内存占用与历史轮次。
- **实用命令** — 通过 `copy` 子命令快速复制系统提示词、上下文片段与压缩会话摘要；通过 `debug` 子命令安全验证编辑操作与预览 Shell 命令。

详细设计与使用方法请参阅[文档](./docs/zh-cn/README.md)。

## 安装

### Windows

**方式一：使用自动化安装脚本（推荐）**

为绕过 PowerShell 执行策略限制，请使用以下方式之一：

- 通过管道将脚本内容传递给 PowerShell（本地克隆）：
  ```powershell
  Get-Content .\scripts\setup-cli.ps1 -Raw | iex
  ```

- 远程一键安装：
  ```powershell
  irm https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.ps1 | iex
  ```

卸载时使用同样的方式，将 `setup-cli.ps1` 替换为 `uninstall-cli.ps1` 即可。

**方式二：手动安装**

1. 本地通过 `cargo build --release` 编译 / 从 [Github Release](https://github.com/SunYanbox/ManualAid-Rust/releases) 下载可执行文件
2. 复制到 `%LOCALAPPDATA%\Programs\ManualAid\` 目录下（用户级安装）或复制到 `C:\Program Files\ManualAid\` 目录下（全局安装）
3. 将上述路径添加到环境变量（用户级添加到用户的 Path 环境变量，全局级添加到全局 Path 环境变量）
4. 重启终端后即可通过 `manualaid-cli.exe` 运行（PowerShell 一般情况只需要输入 `manu` + Tab 即可自动补全）

### Linux

**方式一：使用自动化安装脚本（推荐）**

- 直接运行脚本文件（需本地克隆仓库）：
  ```bash
  bash ./scripts/setup-cli.sh
  ```

- 远程一键安装：
  ```bash
  bash <(curl -fsSL https://raw.githubusercontent.com/SunYanbox/ManualAid-Rust/main/scripts/setup-cli.sh)
  ```

卸载时使用同样的方式，将 `setup-cli.sh` 替换为 `uninstall-cli.sh` 即可。

**方式二：手动安装**

1. 本地通过 `cargo build --release` 编译 / 从 [Github Release](https://github.com/SunYanbox/ManualAid-Rust/releases) 下载可执行文件
2. 复制到 `/usr/local/bin` 目录下 / 如果是本地编译，还可以在项目根目录通过 `sudo install -m 755 target/release/manualaid-cli /usr/local/bin/` 快速安装
3. 通过 `manualaid-cli` 运行（一般只需要输入 `manu` + Tab 即可自动补全）

### 约束

- **提交/暂存 `*.rs` 文件前**，**必须**通过以下三项本地检查：`cargo fmt`（格式化）、`cargo clippy -- -D warnings`（lint）与 `cargo check`（编译）；跨平台全量检查（格式、lint、编译、文档、测试、覆盖率）由 PR 的 CI（`.github/workflows/ci.yml`）承担。
- **交付/PR 前评估覆盖率**：代码覆盖率 Function、Line、Region 三项总体均需**≥80%**（单文件建议≥85%，核心模块≥95%），除非相关文件由于各种原因难以提升覆盖率。仅查看覆盖率信息时优先读取缓存 `coverage_with_lines.txt`，代码未更改时不必重跑全量覆盖率测试。
- 贡献流程与 PR 规范（分支命名、CHANGELOG 更新、PR 模板）请参阅[贡献指南](./CONTRIBUTING_ZH_CN.md)与 `docs/issue-pr-guide.md`。
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
