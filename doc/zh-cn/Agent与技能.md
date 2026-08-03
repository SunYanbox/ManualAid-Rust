---
涉及全局路径: ~/.agents/, ~/.claude/, ~/.codex/, ~/.agent/
涉及Agent配置: Agent TOML 配置 (存放于上述用户目录的 agents 子目录下)
涉及Skill配置: Skill Yml-Markdown 配置 (存放于上述用户目录的 skills 子目录下)
---

# Agent 与技能

## Agent 定义

>  参考[Agent规范](./规格/Agent规范.md)
>
> 此系统实现优先级靠后

- 系统内置 Agent：Plan Agent、Explore Agent、通用 Agent。
- 用户可自定义 Agent，通过含YAML Markdown文件定义，格式包含：
  - name（系统提示词引用名）
  - system_prompt（系统提示词内容）
  - tools（可用工具列表，白名单/黑名单机制。均为空表示支持所有工具；仅白名单为空表示除了黑名单工具外其他均可用）
  - description（可选，面向人类的描述）
  - 等
- 不支持 Agent 间相互调用或委派，用户手动切换。
- 不支持多 Agent 协作。

## 技能（Skill）

> 参考[Skill规范](./规格/Skill规范.md)

- 技能文件使用 YAML + 提示词格式，位于用户目录的 `.agents` 、 `.claude` 、`.codex`、`.agent`等文件夹的skills目录下。
- 技能主文件（SKILL.md）定义开头的Yaml部分由两个`---`括出来，最少需要包含name和description(支持以`>`开头的多行描述)字段。在第二个`---`之后为SKILL的正文Markdown文本。
- 默认只将技能名称和描述加载到系统提示词中。Agent 需要时调用 Skill 工具，系统才会把技能完整提示词注入到当前工具调用链路的上下文（不影响后续会话，除非再次调用）。
- 技能无参数。
- 支持热加载，不支持版本管理。
- 技能与 Agent 无强制绑定关系。
