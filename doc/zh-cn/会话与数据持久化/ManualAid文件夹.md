---
涉及全局路径: ~/.ManualAid/
涉及项目路径: {项目绝对路径}/.ManualAid/
---

# ManualAid文件夹

ManualAid文件夹指`.ManualAid`，用于储存配置文件、记忆文件、数据库以及日志等数据。其中分为：

全局ManualAid文件夹：`~/.ManualAid/`
项目ManualAid文件夹：`{项目绝对路径}/.ManualAid/`

## 全局ManualAid文件夹

上述全局ManualAid文件夹的`~`并不严格匹配Linux的根目录，而是代指当前用户文件夹路径。
这是为了避免数据库、配置文件和日志等过于分散的设计。这一点也参考了OpenCode、ClaudeCode等Agent工具的设计。

- Windows：`C:/Users/{用户名}/.ManualAid/`
- macOS: `/Users/{用户名}/.ManualAid/`
- Linux: `/home/{用户名}/.ManualAid/`
