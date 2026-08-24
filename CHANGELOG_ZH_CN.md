# ChangeLog（开发者版）

本文件记录相对上一版本的完整技术变更，面向开发者；面向用户的简明版见 `docs/changelog/CHANGELOG_ZH_CN.md`。

## [0.3.0] - 2026-08-14

### 解析器与工具过滤

- 新增 `EnabledToolSet`，快照当前启用的工具及其参数名，解析器只识别集合内工具与参数
- `ToolCallFormatParser::try_parse` 改为接收 `&EnabledToolSet`
- `FormatRegistry::parse` 返回 `ParseOutcome`，包含调用列表与软警告
- `FormatRegistry` 新增 `set_enabled_tools` 缓存，集合不变时复用 `Arc<EnabledToolSet>`
- 重写 XML 扫描器，增加工具过滤与未闭合标签软警告
- JSON codeblock 解析器同步支持工具过滤
- `ParseOutcome` 与 `ParseError` 新增相应字段

### 剪贴板抽象

- 新增 `ClipboardProvider` trait，提供 `read`/`write` 抽象
- 新增 `RealClipboard` 与 `MockClipboard`，支持注入读写错误
- `read_clipboard`/`write_clipboard` 改为委托 `RealClipboard`，保持向后兼容
- 测试改为通过 `MockClipboard` 隔离真实系统剪贴板

### CLI 功能

- 新增意图规则复制命令与 `prompt.system.intent-output-rule` 模板
- 新增生成系统提示词与轮次执行的 Token 估算显示
- i18n 文案新增相关键

### 测试组织

- 将 `handlers.rs` 的部分测试拆分到 `tests/commands/handlers.rs`
- 更新剪贴板、解析器相关测试

## [0.2.0] - 2026-08-12

### 会话记录与统计

- 在 `manualaid-ws/src/session.rs` 新增 `RoundStats`，记录每轮解析、审批、执行耗时与估算 Token 总量
- `SessionLog::push` 改为接收 `RoundStats`，并新增 `MAX_ROUNDS` 上限，超过 200 轮时丢弃最旧记录
- 新增 `MemoryUsage` 和 `SessionLog::memory_usage()`，估算内存中会话记录的调用、结果与元数据字节数
- 新增 `SessionLog::rounds()` 返回全部轮次
- `ToolResult` 新增 `execution_duration_ms` 和 `estimated_tokens` 字段
- `Executor::execute` 测量工具执行耗时并写入结果
- `execute_round_with_approval` 返回 `RoundStats`，新增 `estimate_round_tokens` 估算整轮 Token 消耗

### CLI 用户可见功能

- 新增工具历史列表 `/history`，最新在前，显示每轮工具、状态、耗时、Token 以及会话总计
- 复制轮次结果时新增详细预览：显示轮次头、工具详情、耗时与 Token，内容最多预览 10 行
- 配置菜单新增“查看内存会话占用”，展示总占用、解析调用、结果与元数据字节数
- 配置菜单和主菜单新增相应 i18n 文案

### 系统提示词改进

- 在 Windows 上构建提示词时新增 `<platform-notes>` 平台说明
- git 信息块开头新增本地化快照说明
- 格式描述代码块结尾补换行
- 更新 i18n 提示词和工具选择规则文案

### Shell 执行修复

- 在 Windows 上使用 cmd 时通过 `raw_arg` 原样传递命令，避免标准参数转义与 cmd `/C` 引号解析冲突
- 对带引号路径和引号参数的命令额外包裹引号；其他平台保持标准参数转义

### Read 工具预检

- `Executor::pre_check` 新增 Read 工具路径预检，目录和不可读文件在进入审批队列前直接返回失败结果

### 基础设施与发布

- 新增 CI 路径过滤，减少不必要运行
- 新增 Codecov 配置，将覆盖率门槛固定为 80%
- 新增自动化发布工作流，构建并附带各平台二进制
- 新增 Windows 和 Linux 安装/卸载脚本 `scripts/setup-cli.*`、`scripts/uninstall-cli.*`
- 更新 README 徽章与安装说明
- 新增 `doc/zh-cn/提示词设计.md`

## [0.1.0] - 2026-08-09

### 工作区与基础设施

- 初始化 Rust workspace，包含 `i18n`、`manualaid-core`、`manualaid-ws`、`manualaid-cli` 四个 crate
- 新增 `.github/workflows/ci.yml`、`deny.toml`、`.gitignore`
- 新增中英文 `CONTRIBUTING`、`README` 和第三方规范许可证
- 新增 155 个文件，约 24603 行代码、测试与文档

### i18n

- 新增基于 `rust_i18n` 的翻译封装库，提供 `t_str`
- 新增 `audit`、`cli`、`common`、`prompts`、`tools` 五类中英文 locale 文件
- 新增 `build.rs` 与翻译测试

### manualaid-core

- 新增统一错误类型 `error.rs`
- 新增用户标准目录查询 `user_dir.rs`
- 新增剪贴板抽象，提供真实与 Mock 实现 `clipboard.rs`
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
- 新增大量模块测试与集成测试

### manualaid-ws

- 新增工作区配置加载与持久化 `config.rs`
- 新增上下文文件选择 `context.rs`
- 新增系统提示词构建 `prompt.rs`
- 新增会话日志记录 `session.rs`
- 新增对应测试

### manualaid-cli

- 新增命令行参数解析与入口 `cli.rs`、`main.rs`、`lib.rs`
- 新增控制台捕获输出 `console.rs`、终端样式 `style.rs`、分页器 `pager.rs`
- 新增目录树渲染 `dir_tree.rs`、环境路径辅助 `env.rs`
- 新增子命令：`init`、`dir`、`mask`、`restore`、`skill`
- 新增交互式 Agent Copy-Paste Loop：
  - 主循环、数字菜单、配置菜单与上下文选择
  - 审批队列与人工批准
  - 系统提示词生成、粘贴提交、结果复制
  - 会话历史、工具历史与会话摘要
  - inline 快捷命令
  - 审批预览与 diff 显示
- 新增大量单元测试与集成测试

### 文档

- 新增 `doc/zh-cn/` 中文设计文档，覆盖项目概述、核心工作流、工具系统、权限审核、隐私掩码、会话持久化、内置命令、平台适配与 Agent/MCP/Skill 规范
