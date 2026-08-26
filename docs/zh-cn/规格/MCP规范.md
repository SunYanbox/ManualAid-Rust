<!--
  SPDX-License-Identifier: CC-BY-4.0
  版权和许可信息见 THIRD_PARTY/MCPSpecificationLicense.md
-->
## MCP规范

主流规范地址：`https://modelcontextprotocol.io/specification/2026-07-28`

模型上下文协议 (MCP) 是一个开放协议，旨在实现 LLM 应用程序与外部数据源及工具之间的无缝集成。无论您是在构建 AI 驱动的 IDE、增强聊天界面，还是创建自定义的 AI 工作流，MCP 都提供了一种标准化的方式，将 LLM 与其所需的上下文连接起来。

**协议性质**：开放协议，标准化 LLM 应用与外部数据源/工具的集成。

**通信基础**：JSON-RPC 2.0（请求、响应、通知）。

### 1. 核心角色
*   **Hosts**：发起连接的 LLM 应用。
*   **Clients**：Host 内的连接器。
*   **Servers**：提供上下文和能力的服务。

### 2. 服务器功能 (Servers)
可向客户端提供以下**可选**能力：
*   **Resources**：供用户或模型使用的上下文和数据。
*   **Prompts**：用户的模板化消息与工作流。
*   **Tools**：供 AI 模型执行的函数。

### 3. 客户端功能 (Clients)
*   **Elicitation**：服务器可主动向用户请求额外信息。

### 4. 官方扩展 (Extensions)
均为可选，需双方明确支持：
*   **Tasks**：长时操作的异步执行（轮询、中途输入、持久句柄）。
*   **Skills over MCP**：供代理工作流使用的结构化指令。
*   **MCP Apps**：对话中内联的交互式 UI（图表、表单等）。

### 5. 安全与信任 (Security)
1.  **用户同意与控制**：所有数据访问和操作必须经用户明确知晓并授权。
2.  **数据隐私**：未经用户同意，不得将资源数据透传给服务器或第三方。
3.  **工具安全**：工具即代码执行，其描述（如注解）默认不可信；调用前必须获得用户授权。

**实现者应**：构建完善的授权流程，提供清晰的安全文档，遵循隐私最佳实践。

> 本文档基于 Model Context Protocol (MCP) 官方规范（https://modelcontextprotocol.io/specification/2026-07-28）整理而成。
> 原文版权归 Model Context Protocol a Series of LF Projects, LLC 所有。
> 整理版本版权归 github.com/SunYanbox 所有，且保持遵循CC-BY-4.0协议。
