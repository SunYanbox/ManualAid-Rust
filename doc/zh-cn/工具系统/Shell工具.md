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

此处可以使用[which-terminal](https://github.com/ahaoboy/which-terminal) crate快速拿到当前终端信息。
