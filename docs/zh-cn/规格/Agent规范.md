<!--
  SPDX-License-Identifier: MIT
  版权和许可信息见 THIRD_PARTY/AgentsSpecificationLicense.md
-->
# Agent规范

此处参考[OpenCode/配置/Agent](https://opencode.ai/docs/agents/#markdown)

通过Marndown文件定义，一般为`{Agent名称}.md`

文件头部包含以下关键字段：

| 字段 | 类型 | 描述 |
| :--- | :--- | :--- |
| **`name`** | `string` | Agent的显示名称。 |
| **`description`** | `string` | 描述Agent的用途，用于决定何时调用此Agent。 |
| **`mode`** | `enum` | Agent的运行模式，可选 `primary`、`subagent`、`all`。 |
| **`model`** | `string` | 为当前Agent指定特定的模型（如 `anthropic/claude-sonnet-4`）。 |
| **`temperature`** | `number` | 控制模型回复的随机性，范围0.0-1.0。 |
| **`maxSteps`** | `number` | Agent执行任务的最大迭代步数。 |
| **`tools`** | `object` | 启用或禁用特定工具，如 `read`, `edit`, `shell`, `skill`。 |
| **`permission`** | `object` | 设置工具的权限，如 `ask` (询问)、`allow` (允许)、`deny` (拒绝)。 |
| **`disable`** | `boolean` | 是否禁用此Agent。 |

**一个简单的示例**：
```markdown
---
description: 代码审查专家，负责检查代码质量和安全问题
mode: subagent
tools:
  read: true
  grep: true
  edit: false
permission:
  read: allow
  shell: ask
---

你是代码审查专家，请遵循以下原则进行检查...
```

> 本文档基于 OpenCode 官方 Agent 规范文档（https://opencode.ai/docs/agents/#markdown）整理而成。
> 原文版权归 Anomaly 所有，并遵循 MIT 许可证。
> 整理版本版权归 github.com/SunYanbox 所有，且保持遵循 MIT 协议。
