---
涉及全局配置: shell_paths
涉及可用crate: which-terminal
---
# Shell工具设计

**为何不内嵌brush crate**: brush库虽然可以实现内置统一的跨平台Bash终端，但体积过大（约么会额外提供400多个依赖crate树）

## 1. 核心设计理念
- **零嵌入**：不携带任何 Shell 解释器（如 Rust-brush 库），保持工具包体积极小。
- **智能适配**：根据操作系统和当前环境，自动选择最优的 Shell。
- **透明反馈**：执行命令前，**必须明确告知用户当前使用的具体 Shell 类型及路径**（应用启动时，同时会影响Shell工具的提示词），避免因语法差异导致的困惑。
- **可配置覆盖**：允许用户通过配置文件手动指定 Shell 路径，绕过自动检测。

## 2. 全局环境检测优先级（总原则）
在任何平台上，均按照以下**优先级顺序**查找可用的 Shell：

1. **用户显式配置**（最高优先级）：配置文件中的 `shell_paths`。（应当指出配置多个Shell环境，供用户自选）
2. **第三方增强 Shell**：检测 `brush` 是否存在。
3. **系统原生优质 Shell**：检测各平台推荐的默认 Shell（如 Linux 的 Bash，macOS 的 Zsh）。
4. **系统遗留/兼容 Shell**：回退到 `sh` 或 Windows 的 `cmd`。

此处可以使用[which-terminal](https://github.com/ahaoboy/which-terminal) crate（获取结果受环境变量影响显著）快速拿到当前终端信息。如果获取失败，则只声明平台类型。

在 Windows 上可以通过以下命令检测：

```powershell
winget list --id Microsoft.Coreutils --exact
```

该命令在 cmd 与 PowerShell 中均可直接使用。通过它可检测用户是否已安装 Microsoft.Coreutils 扩展，该扩展支持 cat、ls、grep 等类 Unix 命令；如果检测到该扩展，则告知 LLM 当前命令行可以使用类 Unix 命令。该扩展并不完全兼容所有参数格式，具体兼容性取决于对应终端（cmd、PowerShell）。

## 3. Shell 工具参数与审核语义

Shell 工具对应 `ToolKind::Shell`，参数如下：

| 参数 | 类型 | 必填 | semantic | 说明 |
|------|------|------|----------|------|
| `command` | string | 是 | `Command` | 要执行的命令；需匹配白名单，非白名单进入审核列表 |
| `description` | string | 否 | `None` | 命令用途说明，用于审核展示 |
| `timeout` | integer | 否 | `None` | 最大执行毫秒数（默认 120000，最大 600000） |

工作目录不通过参数指定：Shell 工具始终在进程当前工作目录（cwd）下执行，用户通过 `cd path` 指定工作区后再启动 loop。

旧版 Bash 工具中的 `run_in_background`、`dangerously_disable_sandbox` 等参数随 brush 引擎一并废弃，不迁移。
