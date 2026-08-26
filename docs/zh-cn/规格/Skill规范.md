<!--
  SPDX-License-Identifier: CC-BY-4.0
  版权和许可信息见 THIRD_PARTY/SkillsSpecificationLicense.md
-->
# Skill规范

主流规范地址：`https://agentskills.io/specification`

## 目录结构

技能是一个目录，至少包含一个文件：SKILL.md

skill-name/
├── SKILL.md          # Required: metadata + instructions
├── scripts/          # Optional: executable code
├── references/       # Optional: documentation
├── assets/           # Optional: templates, resources
└── ...               # Any additional files or directories

## SKILL.md格式

该文件必须包含 YAML 前置元数据（frontmatter），后接 Markdown 内容。`SKILL.md`

**前置元数据（Frontmatter）**

| 字段 | 是否必填 | 约束条件 |
| :--- | :--- | :--- |
| `name` | 是 | 最多 64 个字符。仅限小写字母、数字和连字符。不能以连字符开头或结尾。 |
| `description` | 是 | 最多 1024 个字符。不可为空。描述该技能的功能及使用时机。 |
| `license` | 否 | 许可证名称或对捆绑许可证文件的引用。 |
| `compatibility` | 否 | 最多 500 个字符。指明环境要求（目标产品、系统软件包、网络访问权限等）。 |
| `metadata` | 否 | 用于附加元数据的任意键值映射。 |
| `allowed-tools` | 否 | 以空格分隔的字符串，列出该技能可使用的预批准工具。（实验性功能） |

## 渐进式披露（Progressive disclosure）

代理（Agents）会**渐进式**地加载技能，仅在任务需要时才会拉取更多详细信息。技能的结构应充分利用这一点：

1. **元数据（Metadata）**（约 100 个 tokens）：`name`（名称）和 `description`（描述）字段会在启动时为所有技能加载。
2. **指令（Instructions）**（建议少于 5000 个 tokens）：`SKILL.md` 的完整正文会在技能被**激活**时加载。
3. **资源（Resources）**（按需加载）：文件（例如位于 `scripts/`、`references/` 或 `assets/` 目录下的文件）仅在需要时才会被加载。

请将主 `SKILL.md` 文件保持在 **500 行以内**。将详细的参考材料移至单独的文件中。

> 本文档基于 Agent Skills 官方规范文档（https://agentskills.io/specification）整理而成。
> 原文版权归 Anthropic, PBC 所有，并遵循 CC-BY-4.0 许可证。
> 整理版本版权归 github.com/SunYanbox 所有，且保持遵循 CC-BY-4.0 协议。
