# ChangeLog（开发者版）

本文件记录相对上一版本的完整技术变更，面向开发者；面向用户的简明版见 `docs/changelog/CHANGELOG_ZH_CN.md`。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，并遵循语义化版本。

## [Unreleased]

### 变更

- `<directory_listing>` 块开头新增本地化快照说明，提醒 LLM 目录结构为启动时快照不会后续更新；修复闭合标签前缺换行的问题

## [0.8.0] - 2026-08-25

### 新增

- 主菜单输入框支持 `!` 前缀直接运行 Shell 命令，复用轮次结果显示与历史记录
- 设置菜单新增内置更新日志查看器，支持按版本和一次查看全部
- `i18n` 新增 `changelog_all`、`changelog_versions`、`changelog_version` 公开接口，并封装 `set_locale`
- 菜单项新增稳定唯一键（slug），供调用方与测试选择菜单项

### 变更

- 工具输出分页改为每页三行
- 审批预览与 Diff 输出分页改为首次 20 行、之后每次 10 行

## [0.7.0] - 2026-08-21

### 新增

- 新增 `menu.rs` 和 `command.rs`，统一数字菜单与 inline 命令处理
- 工具结果使用方括号包裹工具名，并显示重要参数摘要
- Read 输出末尾追加范围/行数标记
- 未闭合工具调用保留为失败结果，不再静默丢弃
- 默认解析器顺序改为 json-codeblock、invoke、xml
- Windows 平台说明新增 CoreUtils 提示
- 配置菜单新增复制提示词子菜单，支持复制意图规则、工具格式、启用工具列表、行尾处理规则、计划模式规则、切换执行模式规则和任务规划规则

### 变更

- 工具开关独立为三级子菜单
- `skill` 模块拆分为 `config_io`、`frontmatter`、`scan` 等子模块
- 大量实现与测试按模块目录拆分，提升可维护性
- 更新提示词与工具描述

## [0.6.0] - 2026-08-18

### 新增

- 新增 invoke 线格式解析器，支持 `<invoke name="tool"><parameter name="param">value</parameter></invoke>` 格式
- `ToolCallFormat` 新增 `Invoke` 变体，`/format` 循环顺序和配置支持 `invoke`

### 修复

- 修复 XML 解析器参数值中裸 `<` 后接空白或非法名称起始字符时误吞后续闭合标签的问题

## [0.5.0] - 2026-08-18

### 新增

- 新增 `debug whitelist` 命令，分层查看审计命令白名单的默认值、项目/全局配置、合并生效列表与黑名单冲突
- 新增 inline 快捷命令：`/help`、`/history`、`/summary`、`/clear`、`/mode`，并支持 `/h`、`/H`、`/s`、`/cls`、`/m` 等别名
- 主循环首次渲染菜单后显示帮助提示，并在提示符前显示当前审批模式
- 启动消息包含版本号
- JSON 解析器接受系统提示词使用的 `func_calls` 围栏

### 变更

- 大幅扩充内置默认 Shell 白名单，新增 ls、cat、grep、git 系列、gh 系列、cargo 系列以及平台命令
- 对齐 Agent 提示词与工具描述，更新中英文 locale 文案
- 主循环新增 `/mode`、`/m` 快捷切换审批模式
- 配置菜单审批模式标签逻辑移动到 utils

## [0.4.1] - 2026-08-16

### 新增

- Read 工具新增 `show_line_numbers` 和 `show_line_endings` 诊断参数
- 新增 `AGENTS.md` 开发指南
- 新增 `docs/comment-style.md`、`docs/commit-conventions.md`、`docs/issue-pr-guide.md` 文档
- 新增 `scripts/ci.cmd`、`scripts/ci.ps1`、`scripts/ci.sh` CI 检查脚本

### 变更

- Edit `old_string` 未找到时，附加换行符差异提示或相似度不低于 90% 的候选字符串
- 更新 Read/Edit 工具的中英文描述，移除默认 2000 行说明并补充诊断参数说明

## [0.4.0] - 2026-08-15

### 新增

- 新增 `debug` 子命令组，将原 `mask`、`restore`、`skill` 子命令迁移为其子命令
- 新增 `debug plan_edit`：通过真实校验路径预检 Edit `old_string` 是否匹配，报告出现次数与搜索文本，不修改文件
- 新增 `debug shell`：预览、确认并执行 Shell 命令，展示 stdout、stderr、退出码与耗时
- `debug plan_edit` 和 `debug shell` 支持 `@文件路径` 语法从文件读取内容参数
- 新增 `docs/zh-cn/工具系统/关于XML解析器的逻辑.md` 文档

### 变更

- `plan_edit` 与 `EditPlan` 对外公开，供调试命令复用真实执行校验路径
- `plan_edit` 的 `new_string` 缺失时默认空字符串
- XML 解析器严格 CDATA 闭合规则：`]]>` 与闭合标签之间必须紧贴，不允许空白；未闭合参数标签写软警告
- 非紧贴 CDATA 按字面文本处理

## [0.3.1] - 2026-08-15

### 新增

- Edit 成功结果新增行数变化和 diff 输出；`replace_all` 时生成文件级 diff，否则生成参数级 diff

### 变更

- XML 解析器 CDATA 包裹清理前导空白，非 CDATA 参数只裁首尾换行符并允许空字符串

### 修复

- Edit `old_string` 未找到时附带原字符串

## [0.3.0] - 2026-08-14

### 变更

- `plan_edit` 与 `EditPlan` 对外公开，供调试命令复用真实执行校验路径
- `plan_edit` 的 `new_string` 缺失时默认空字符串
- Edit 成功结果新增行数变化和 diff 输出；`replace_all` 时生成文件级 diff，否则生成参数级 diff
- XML 解析器严格 CDATA 闭合规则：`]]>` 与闭合标签之间必须紧贴，不允许空白；未闭合参数标签写软警告
- 非紧贴 CDATA 按字面文本处理，只裁首尾换行符

## [0.3.0] - 2026-08-14

### 新增

- 新增 `EnabledToolSet`，快照当前启用的工具及其参数名，解析器只识别集合内工具与参数
- `FormatRegistry` 新增 `set_enabled_tools` 缓存，集合不变时复用 `Arc<EnabledToolSet>`
- 新增 `ClipboardProvider` trait，提供 `read`/`write` 抽象
- 新增 `RealClipboard` 与 `MockClipboard`，支持注入读写错误
- 新增意图规则复制命令与 `prompt.system.intent-output-rule` 模板
- 新增生成系统提示词与轮次执行的 Token 估算显示

### 变更

- `ToolCallFormatParser::try_parse` 改为接收 `&EnabledToolSet`
- `FormatRegistry::parse` 返回 `ParseOutcome`，包含调用列表与软警告
- 重写 XML 扫描器，增加工具过滤与未闭合标签软警告
- JSON codeblock 解析器同步支持工具过滤
- `read_clipboard`/`write_clipboard` 改为委托 `RealClipboard`，保持向后兼容
- 测试改为通过 `MockClipboard` 隔离真实系统剪贴板
- 将 `handlers.rs` 的部分测试拆分到 `tests/commands/handlers.rs`
- i18n 文案新增相关键

## [0.2.0] - 2026-08-12

### 新增

- 新增 `RoundStats`，记录每轮解析、审批、执行耗时与估算 Token 总量
- `SessionLog::push` 改为接收 `RoundStats`，并新增 `MAX_ROUNDS` 上限，超过 200 轮时丢弃最旧记录
- 新增 `MemoryUsage` 和 `SessionLog::memory_usage()`，估算内存中会话记录的调用、结果与元数据字节数
- 新增 `SessionLog::rounds()` 返回全部轮次
- `ToolResult` 新增 `execution_duration_ms` 和 `estimated_tokens` 字段
- 新增工具历史列表 `/history`，最新在前，显示每轮工具、状态、耗时、Token 以及会话总计
- 复制轮次结果时新增详细预览：显示轮次头、工具详情、耗时与 Token，内容最多预览 10 行
- 配置菜单新增“查看内存会话占用”，展示总占用、解析调用、结果与元数据字节数
- 在 Windows 上构建提示词时新增 `<platform-notes>` 平台说明
- git 信息块开头新增本地化快照说明
- 新增 CI 路径过滤，减少不必要运行
- 新增 Codecov 配置，将覆盖率门槛固定为 80%
- 新增自动化发布工作流，构建并附带各平台二进制
- 新增 Windows 和 Linux 安装/卸载脚本 `scripts/setup-cli.*`、`scripts/uninstall-cli.*`
- 新增 `docs/zh-cn/提示词设计.md`

### 变更

- `Executor::execute` 测量工具执行耗时并写入结果
- `execute_round_with_approval` 返回 `RoundStats`，新增 `estimate_round_tokens` 估算整轮 Token 消耗
- 格式描述代码块结尾补换行
- 更新 i18n 提示词和工具选择规则文案
- 更新 README 徽章与安装说明

### 修复

- 在 Windows 上使用 cmd 时通过 `raw_arg` 原样传递命令，避免标准参数转义与 cmd `/C` 引号解析冲突
- 对带引号路径和引号参数的命令额外包裹引号；其他平台保持标准参数转义
- `Executor::pre_check` 新增 Read 工具路径预检，目录和不可读文件在进入审批队列前直接返回失败结果

## [0.1.0] - 2026-08-09

### 新增

- 初始化 Rust workspace，包含 `i18n`、`manualaid-core`、`manualaid-ws`、`manualaid-cli` 四个 crate
- 新增 `.github/workflows/ci.yml`、`deny.toml`、`.gitignore`
- 新增中英文 `CONTRIBUTING`、`README` 和第三方规范许可证
- 新增基于 `rust_i18n` 的翻译封装库，提供 `t_str`
- 新增 `audit`、`cli`、`common`、`prompts`、`tools` 五类中英文 locale 文件
- 新增统一错误类型 `error.rs`
- 新增用户标准目录查询 `user_dir.rs`
- 新增剪贴板抽象 `clipboard.rs`
- 新增 Shell 执行、超时中止与编码检测 `shell.rs`
- 新增文件锁 I/O 与标准目录初始化 `file_io.rs`、`manualaid_dir.rs`
- 新增技能发现、去重与启用管理 `skill.rs`
- 新增可逆隐私掩码 `privacy/`，包含配置、过滤器与注册表
- 新增计时原语 `timer.rs`
- 新增工具层 `tools/`：Read、Edit、Write、Shell、Skill 及统一 `ToolResult`
- 新增解析层 `parser/`：XML、JSON codeblock、注册表与格式解析器
- 新增审计层 `audit/`：路径边界检查、Shell 白名单/黑名单与内容检查
- 新增执行管线 `executor.rs`：路由、校验、掩码还原、审计、执行、后处理
- 新增工作区路径归一化与边界检查 `workspace.rs`
- 新增异步文件读写 `async_fs.rs`
- 新增工作区配置加载与持久化 `config.rs`
- 新增上下文文件选择 `context.rs`
- 新增系统提示词构建 `prompt.rs`
- 新增会话日志记录 `session.rs`
- 新增命令行参数解析与入口 `cli.rs`、`main.rs`、`lib.rs`
- 新增控制台捕获输出 `console.rs`、终端样式 `style.rs`、分页器 `pager.rs`
- 新增目录树渲染 `dir_tree.rs`、环境路径辅助 `env.rs`
- 新增子命令：`init`、`dir`、`mask`、`restore`、`skill`
- 新增交互式 Agent Copy-Paste Loop：主循环、数字菜单、配置菜单、审批队列、系统提示词生成、粘贴提交、结果复制、会话历史、inline 快捷命令、审批预览与 diff 显示
- 新增 `docs/zh-cn/` 中文设计文档
- 新增大量模块测试与集成测试
