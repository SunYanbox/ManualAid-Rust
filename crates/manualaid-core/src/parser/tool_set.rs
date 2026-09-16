//! The enabled-tool snapshot shared by every parser: which tool names and
//! parameter names are currently defined, with O(1) lookups for both.
//! 解析器共享的可用工具快照：当前定义的工具名与参数名，两者均为 O(1)
//! 查询。
//!
//! # Description
//! Built-in tools come from the compile-time [`ToolKind`] enum and keep their
//! `&'static str` names; MCP tools are discovered at runtime and carry owned
//! names. Both are stored here so the parsers keep seeing one uniform set and
//! stay unaware of where a tool came from.
//! # 描述
//! 内置工具来自编译期 [`ToolKind`] 枚举，保留其 `&'static str` 名称；MCP
//! 工具在运行时发现，携带拥有所有权的名称。两者都存放在这里，使解析器
//! 始终面对同一个集合，且不感知工具来源。

use std::collections::{HashMap, HashSet};

use crate::mcp::McpTool;
use crate::tools::ToolKind;
use crate::tools::all_tools;

/// Snapshot of the available tools and their defined param names. Parsers
/// accept only tools/params present here and discard everything else.
/// 可用工具及其已定义参数名的快照。解析器只接受其中出现的工具与参数，
/// 其余内容一律丢弃。
#[derive(Debug, Clone)]
pub struct EnabledToolSet {
    /// Built-in tool name → kind, for O(1) "is this a defined tool name"
    /// checks and for routing to a `ToolKind`.
    /// 内置工具名 → 工具类型，用于 O(1) 判定字符串是否为已定义工具名，
    /// 以及路由到 `ToolKind`。
    by_name: HashMap<&'static str, ToolKind>,
    /// Built-in tool name → set of its defined param names.
    /// 内置工具名 → 其已定义参数名集合。
    params: HashMap<&'static str, HashSet<&'static str>>,
    /// MCP exposed tool name → set of its defined param names.
    /// MCP 暴露工具名 → 其已定义参数名集合。
    mcp: HashMap<String, HashSet<String>>,
}

impl EnabledToolSet {
    /// The set containing every built-in tool.
    /// 包含全部内置工具的集合。
    pub fn all() -> Self {
        Self::from_tool_kinds(all_tools())
    }

    /// Build from tool kinds; each tool's `parameters()` is consulted once.
    /// 从工具类型构建；每个工具的 `parameters()` 只查询一次。
    pub fn from_tool_kinds(kinds: &[ToolKind]) -> Self {
        let mut by_name = HashMap::with_capacity(kinds.len());
        let mut params = HashMap::with_capacity(kinds.len());
        for &kind in kinds {
            let param_names = kind
                .parameters()
                .into_iter()
                .map(|param| param.name)
                .collect();
            params.insert(kind.name(), param_names);
            by_name.insert(kind.name(), kind);
        }
        Self {
            by_name,
            params,
            mcp: HashMap::new(),
        }
    }

    /// Build from names; names not resolvable by `ToolKind::from_name` are
    /// silently dropped.
    /// 从名称构建；无法由 `ToolKind::from_name` 解析的名称会被静默丢弃。
    pub fn from_names(names: &[String]) -> Self {
        let kinds: Vec<ToolKind> = names
            .iter()
            .filter_map(|name| ToolKind::from_name(name))
            .collect();
        Self::from_tool_kinds(&kinds)
    }

    /// Build from built-in names plus every tool contributed by an enabled
    /// MCP server.
    /// 从内置名称加上每个已启用 MCP 服务器贡献的工具构建。
    pub fn from_names_and_mcp(names: &[String], mcp_tools: &[McpTool]) -> Self {
        let mut set = Self::from_names(names);
        for tool in mcp_tools {
            set.mcp.insert(
                tool.exposed_name.clone(),
                tool.params.iter().map(|param| param.name.clone()).collect(),
            );
        }
        set
    }

    /// Whether `name` is a defined tool in the set, built-in or MCP.
    /// `name` 是否为集合中已定义的工具，内置或 MCP 皆可。
    pub fn contains_tool(&self, name: &str) -> bool {
        self.by_name.contains_key(name) || self.mcp.contains_key(name)
    }

    /// Resolve `name` to its built-in tool kind, if it is in the set.
    /// MCP tools have no `ToolKind` and yield `None`.
    /// 若 `name` 是集合中的内置工具，解析为对应的工具类型。
    /// MCP 工具没有 `ToolKind`，返回 `None`。
    pub fn tool_kind(&self, name: &str) -> Option<ToolKind> {
        self.by_name.get(name).copied()
    }

    /// Whether `param` is a defined param of tool `tool_name`.
    /// `param` 是否为工具 `tool_name` 的已定义参数。
    pub fn contains_param(&self, tool_name: &str, param: &str) -> bool {
        if let Some(names) = self.params.get(tool_name) {
            return names.contains(param);
        }
        self.mcp
            .get(tool_name)
            .is_some_and(|names| names.contains(param))
    }

    /// Names of the tools in the set, used by the registry as the cache
    /// fingerprint: built-ins in canonical `all_tools` order first, then MCP
    /// names sorted, so the same set always produces the same sequence.
    /// 集合内工具的名称，注册表用作缓存指纹：内置工具按 `all_tools` 的规范
    /// 顺序在前，MCP 名称排序在后，使同一集合始终产生同一序列。
    pub fn tool_names(&self) -> Vec<String> {
        let mut names: Vec<String> = all_tools()
            .iter()
            .filter(|kind| self.contains_tool(kind.name()))
            .map(|kind| kind.name().to_string())
            .collect();
        let mut mcp: Vec<String> = self.mcp.keys().cloned().collect();
        mcp.sort();
        names.extend(mcp);
        names
    }
}

impl Default for EnabledToolSet {
    fn default() -> Self {
        Self::all()
    }
}

#[cfg(test)]
mod tests;
