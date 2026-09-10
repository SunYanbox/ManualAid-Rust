# ChangeLog（开发者版）

本文件记录相对上一版本的完整技术变更，面向开发者；面向用户的简明版见 `docs/changelog/CHANGELOG_ZH_CN.md`。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，并遵循语义化版本。

## [Unreleased]

### 新增

- `!` shell 命令、`/SKILL` 技能加载与 `@path` 文件/文件夹引用等用户驱动操作的执行结果统一以 `[USER_ACTION kind="exec|skill|path" …] … [END USER_ACTION]` 包裹回贴给外部 LLM：头部属性按 kind 取 `command`/`name`/`path`，label 分别为完整用户命令、技能暴露名与绝对路径（目录末尾补 `\` 标记）；常规 `[TOOL_RESULT]` 渲染逐字节不变，截断与完整输出暂存管线对两类包裹统一复用
- `ToolResult` 新增可选 `user_action` 字段（`serde(default, skip_serializing_if)`）与链式 `with_user_action(kind, label)` 构造器；`UserActionKind::as_str/attr_name` 集中 kind 线上名与 label 属性名映射，供后续 `/SKILL`、`@path` 接入复用；旧 session JSON 缺省加载为 `None`，无值序列化省略字段
- 系统提示词（中英文）在 system-reminder 说明之后新增 `user-action-note`，向外部 LLM 解释 USER_ACTION 块含义及不要重复执行用户操作
- 主菜单输入新增交互式补全：行首 `/` 显示内置命令与已启用 SKILL（完整唯一名），行首或空格后 `@` 显示项目路径；↑/↓ 选择、Tab 填入、Enter 确认、Esc 关闭建议；`/SKILL` 直接加载技能、`@path` 读取文件或列出文件夹（目录使用树形渲染），并各自产生 `[USER_ACTION kind="skill"|"path"]` 结果；非交互与测试构建自动回退到脚本输入，既有测试机制零侵入
- 补全编辑器新增共享会话输入历史 `InputHistory`：`Mutex` 保护的条目列表跨编辑轮次共享，`push` 去除更早的相同条目使相同输入只保留最新一条，空白行被忽略；`mod.rs` 每会话创建一个实例并通过 `Arc` 传入 `read_line_with_completion`，交互提交与非交互 `read_line` 回退路径都会记录输入
- 补全编辑器新增方向键历史回溯：无建议面板时 ↑ 调出更早的历史条目、↓ 调出更新的条目，首次调出把当前缓冲保存为草稿，越过最新条目时恢复草稿；无历史时上下键为无操作
- 补全编辑器光标升级为字符边界字节索引：`Char` 在光标处插入、`Backspace` 删除光标前一字符、Left/Right 单字符移动、Ctrl+Left/Right 按词移动；crossterm 键位转换映射普通方向键、Ctrl 修饰方向键与 Emacs 别名 C-b/C-f；光标移动触发候选刷新，因为活动补全 token 改为止于光标处
- 补全渲染把光标放在提示符宽度加光标前文本显示宽度的位置，CJK 字符下光标保持对齐；Tab 填入候选后光标停在插入内容之后而非跳到缓冲末尾

### 变更

- 技能工具描述中“Do not invoke a skill that is already running”措辞存在歧义，易被误解为并发执行状态；英文改为 “Do not invoke a skill that is already loaded”，中文由“不要调用已在运行的技能”改为“不要调用已经加载过的技能”，明确指代当前对话中已加载/激活的技能

## [0.12.0] - 2026-09-09

### 新增

- 会话压缩提示词（中英文）新增“情绪净化”原则：摘要剥离脏话、人身攻击等侮辱性情绪化措辞，仅保留中性技术信息并以一句中性话概括用户不满；“保留决策链”改为引用用户技术性原话；Critical Context 的 [禁区]/[硬约束] 项要求剥离情绪化用词
- 新增 `terminal_title` 模块：交互式 loop 启动时用 `crossterm::terminal::SetTitle` 把终端窗口标题设为 `[ManualAid] <项目文件夹名>`（无可用文件夹名时回退 `[ManualAid]`，文件夹名中的控制字符被剔除以免破坏 OSC 序列）；标题写入经由 `console` 出口，stdout 非终端或处于测试捕获时不写入真实终端

### 变更

- 技能唯一名称系统重构：`Skill::unique_name` 恒为稳定、可追溯来源的 `<scope>-<agent_dir>-<name>`（如 `project-.agents-pdf`）；`Skill` 新增 `agent_dir` 字段，扫描器在加载时直接赋值。同名但内容不同的技能不再被改成 `.project-`/`.global-` 前缀，而是各自保留稳定名；同一扫描根内声明相同 frontmatter `name` 的残余冲突以确定性 `-N` 后缀解决，且后缀候选会避开其他技能的自然名（名为 `pdf-2` 的真实技能不会被遮蔽）
- 新增暴露名解析：`exposed_name_map` 与 `resolve_skill` 按*已启用*技能集合计算最短唯一名称（无冲突时裸名、同名加 `<agent_dir>-` 前缀、同目录跨作用域用完整稳定名）；`<available_skills>` 提示列表与 Skill 工具参数改用暴露名，`resolve_skill` 仍可解析禁用技能的完整稳定名以便报告“已禁用”；`get_skill` 收紧为仅匹配完整稳定名
- 技能重复检测对空白与行尾宽容：`description`/`body` 在比较前去除首尾空白并把 `\r\n` 归一化为 `\n`，存储文本不被改写
- Skill 工具“未找到”提示与 CLI 管理界面（`debug skill`、loop CLI 技能菜单）随之更新：提示列出暴露名，管理界面始终显示完整稳定唯一名称

## [0.11.0] - 2026-09-07

### 新增

- 编辑工具在 `old_string` 未匹配时仅返回针对性诊断：仅换行符差异时直接给出对应行尾风格与调整建议，存在高度相似候选时只返回候选文本，不再叠加“未找到、请重新读文件”的基础提示，避免干扰模型判断

### 修复

- 移除系统提示中“Windows 下使用正斜杠路径”的建议，避免该指引引发路径解析失败

## [0.10.3] - 2026-09-05

### 新增

- Skill 工具成功执行时，返回的 `invoke_skill` JSON 头部新增 `path` 字段，值为技能文件夹绝对路径（`/` 分隔），供 Agent 解析技能内 `scripts/`、`references/` 等相对资源路径
- 编辑工具行尾不一致警告增加可执行提示：直接指引将 `old_string` 与 `new_string` 切换为文件匹配片段实际使用的 CRLF/LF 后重试
- 读取工具页脚自动报告文件行尾风格（LF/CRLF/混合），无需单独开启逐行诊断即可获知编辑所需行尾信息

### 修复

- 修正混合行尾文件下编辑工具行尾差异检测对匹配片段换行符的误判

## [0.10.2] - 2026-09-04

### 新增

- 新增 `.github/workflows/changelog-reminder.yml` 工作流：在 PR 检测到 `crates/`、`Cargo.toml`、`Cargo.lock` 或 `.github/` 目录变更时，若未更新 `CHANGELOG*.md` 则发布提醒评论；若后续更新了 CHANGELOG，则自动将已有评论更新为通过状态。

### 修复

- 简化中英文默认拒绝提示为单纯的拒绝声明（`操作已被用户拒绝` / `Operation denied by user`），移除命令内容、白名单状态与批准要求等冗余描述（命令信息已通过结果参数摘要提供）

## [0.10.1] - 2026-08-30

### 变更

- 意图输出规则从 `<rules>` 内移出，改为在规则块前渲染的独立 `<system-reminder>`；规则内容替换为新的 `# Intent`/`# ToolCall`/`# Clarify`/`# Answer` 格式，并移除 `capabilities` 中重复的意图说明

## [0.10.0] - 2026-08-27

### 新增

- `manualaid-cli` 新增 `copy` 子命令，无需进入 TUI 即可复制 10 种提示词片段；其中 `system-prompt` 与 `context` 支持 `--context-files` 覆盖开关（取值 `all`/`none`/`First`，默认 `First`），`--lang` 对子命令生效，剪贴板写失败返回非零退出码

## [0.9.0] - 2026-08-26

### 新增

- 系统提示词 `path-rules` 新增 @ 前缀文件读取规则
- 复制提示词二级菜单新增“复制上下文”，复用多上下文文件选择询问
- `manualaid-ws::prompt` 新增 `render_context_reminder` 公开函数，供系统提示词构建与复制上下文菜单共用
- 复制提示词二级菜单新增“复制压缩会话提示词”，复制固定压缩会话提示词模板到剪贴板
- 工具结果超出 `max_result_chars` 触发截断时，完整未截断输出写入 `<workspace_root>/.ManualAid/temp/<sha256>.md`，并在截断文本中追加本地化通知告知暂存路径与各工具输出起始行号

### 变更

- `<directory_listing>` 块开头新增本地化快照说明，提醒 LLM 目录结构为启动时快照不会后续更新；修复闭合标签前缺换行的问题
- JSON 代码块工具调用模板的行尾转义说明细化：区分 LF 与 CRLF 的转义写法
- 工作区上下文文件从 `<dynamic-context>` 移出，改为在 `</system_prompt>` 后输出独立 `<system-reminder>` 块；`render_context_files` 输出改为按文件的 `Instructions from` 小节，文件标签本地化
- 系统提示词 `path-rules` 移除对 `<context_files>` 的路径来源引用

### 修复

- 补齐中文系统提示词 `path-rules` 缺失的目录列出与 Windows 正斜杠规则，与英文版对齐

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
