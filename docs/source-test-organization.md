# 源码与测试组织

本文件说明 ManualAid 工作区的 crate 划分、源码目录规则与测试放置方式。编写代码或测试前必须阅读本文件。

## Workspace 结构

`Cargo.toml` 定义的 workspace 包含四个 crate：

| Crate | 职责 |
|-------|------|
| `crates/i18n` | 基于 `rust-i18n` 的翻译封装。`i18n!()` 只在此库 crate 调用一次；二进制使用 `i18n::t_str()`。 |
| `crates/manualaid-core` | 核心领域库：工具定义与执行、审计、解析器、隐私、技能、工作区路径等，不含交互式 UI。 |
| `crates/manualaid-ws` | 面向 CLI 循环的工作区配置、系统提示词构建与内存会话记录。 |
| `crates/manualaid-cli` | 可执行程序及其库（lib）：命令行解析、展示格式化、隐私快照 CLI 入口、pager。 |

依赖方向大致为：`manualaid-ws` 依赖 `manualaid-core` 与 `i18n`；`manualaid-cli` 依赖 `i18n`、`manualaid-core` 与 `manualaid-ws`；`i18n` 是其余 crate 共享的翻译层。

## manualaid-core 源码组织

顶层模块在 `crates/manualaid-core/src/lib.rs` 中声明：

```rust
pub mod async_fs;
pub mod audit;
pub mod clipboard;
pub mod error;
pub mod executor;
mod file_io;
pub mod manualaid_dir;
pub mod parser;
pub mod privacy;
pub mod shell;
pub mod skill;
pub mod timer;
pub mod tools;
pub mod user_dir;
pub mod workspace;
```

模块职责（依据各模块文档注释）：

- `async_fs`：基于 `tokio::fs` 的异步文件读写，错误映射到统一错误类型。
- `audit`：通过 `ParamSemantic` 审计工具参数，不硬编码工具名；包含 `strategies` 子模块。
- `clipboard`：剪贴板抽象（`ClipboardProvider` trait），提供真实与 Mock 实现。
- `error`：整个库的统一错误类型 `CoreError`。
- `executor`：执行管线：路由 → 校验 → 还原掩码 → 审计 → 执行 → 后处理。
- `file_io`：`.ManualAid/*` 文件的加锁 I/O（进程内 mutex + 跨进程 NamedLock）。
- `manualaid_dir`：标准目录与文件的幂等创建。
- `parser`：工具调用解析（XML、JSON code block），包含 `invoke`、`json_codeblock`、`registry`、`tool_set`、`xml` 等子模块。
- `privacy`：隐私掩码与还原；包含 `config`、`filter`、`registry` 子模块。
- `shell`：可配置 Shell 执行、超时中止与编码处理。
- `skill`：技能发现、去重与启用/禁用管理。
- `timer`：基于 `std::time::Instant` 的计时原语。
- `tools`：工具定义与执行，`ToolKind` 枚举是提示词构建器与执行器共享的事实来源。
- `user_dir`：查询用户标准目录（home、config、cache 等）。
- `workspace`：路径归一化、工作区边界检查与豁免合并。

### 文件与目录同名拆分

当一个模块实现过长或测试需要按主题拆分时，采用“同名 `.rs` + 同名子目录”的形式：

```text
src/executor.rs
src/executor/tests/mod.rs
src/executor/tests/coerce.rs
src/executor/tests/error.rs
...
```

`executor.rs` 末尾通过 `#[cfg(test)] mod tests;` 引入 `src/executor/tests/` 下的测试：

```rust
#[cfg(test)]
mod tests;
```

`src/executor/tests/mod.rs` 再声明各个测试文件，例如：

```rust
mod coerce;
mod error;
mod execute;
mod type_name;
mod validate;
```

这套规则同样适用于 `parser` 与 `tools` 下的子模块：`src/parser/xml.rs` 的实现对应 `src/parser/xml/tests/` 测试，`src/tools/read.rs` 对应 `src/tools/read/tests/`。

### 单文件模块的内部测试

当模块是单个 `.rs` 文件、测试量不大时，可以不建目录，而用 `#[path]` 指向平级测试文件：

实现文件 `src/privacy/filter.rs` 末尾：

```rust
#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
```

测试文件 `src/privacy/filter_tests.rs` 开头使用：

```rust
use super::*;
```

## manualaid-cli 源码组织

`crates/manualaid-cli/src/lib.rs` 声明以下模块：

```rust
pub mod cli;
pub mod commands;
pub mod console;
pub mod dir_tree;
mod env;
pub mod pager;
pub mod style;
pub mod terminal_title;
```

- `cli`：CLI 参数与入口编排。
- `commands`：交互式命令，包含 `debug` 与 `loop_cli` 子模块。
- `console`：控制台输出辅助。
- `dir_tree`：目录树渲染。
- `env`：环境路径辅助。
- `pager`：长输出分页。
- `style`：终端样式辅助。
- `terminal_title`：终端窗口标题（以项目文件夹名标识会话）。

`commands` 中的单文件模块遵循与 core 相同的 `_tests.rs` 模式，例如 `src/commands/dir.rs` 末尾：

```rust
#[cfg(test)]
#[path = "dir_tests.rs"]
mod tests;
```

对应测试文件为 `src/commands/dir_tests.rs`，其中 `use super::*;`。

## manualaid-ws 与 i18n 源码组织

- `manualaid-ws/src/lib.rs` 声明 `config`、`context`、`prompt`、`session` 四个模块，分别处理配置、上下文、系统提示词构建与会话记录。
- `i18n/src/lib.rs` 是翻译封装；`i18n/src/init.rs` 仅含一次编译期宏调用。

## 测试放置规则

### 公共 API：crate 级 tests 目录

涉及公共 API 的测试必须放到对应 crate 的 `tests/` 目录，避免实现与测试混在同一源文件。

各 crate 集成测试目录：

- `crates/manualaid-core/tests/`：与 `src` 模块一一对应的测试文件，另有 `tests/common/mod.rs` 提供共享测试辅助。
- `crates/manualaid-cli/tests/`：按 API 和命令维度拆分；`tests/api.rs` 与 `tests/commands.rs` 用 `#[path]` 聚合对应子目录。
- `crates/manualaid-ws/tests/`：`config.rs`、`context.rs`、`prompt.rs`。
- `crates/i18n/tests/`：`test_i18n_lib.rs`。

### pub(crate) 或私有 API：模块内测试

对 `pub(crate)` 或私有 API，测试应随实现一起放在源文件附近，并在实现文件末尾声明：

- 目录形态（实现拆分到目录）：`#[cfg(test)] mod tests;`，测试在 `<name>/tests/`。
- 文件形态（实现仍是单文件）：`#[cfg(test)] #[path = "<source_name>_tests.rs"] mod tests;`，测试在 `<source_name>_tests.rs`，开头 `use super::*;`。

选择目录形态还是 `_tests.rs` 形态，取决于测试数量与主题是否需要拆分为多个文件。默认优先少建目录，测试主题多、文件长时再拆目录。

### 测试与输出规则

测试编写还需遵循 AGENTS.md 中的以下要求：

- stdout 测试重定向：涉及向 stdout 输出的测试要写到内部字符串变量再断言，避免污染终端。
- 剪贴板测试：优先使用 `MockClipboard`；禁止默认 CI 下直接操作真实剪贴板并断言内容。
- 覆盖率：单个文件尽量≥85%，核心模块尽量≥95%；总体 Function/Line/Region 均≥80%。
- 覆盖率豁免：`crates/manualaid-core/src/user_dir.rs` 与 `crates/i18n/src/init.rs` 暂不纳入覆盖率提升。
