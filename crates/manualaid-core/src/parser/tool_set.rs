//! The enabled-tool snapshot shared by every parser: which tool names and
//! parameter names are currently defined, with O(1) lookups for both.
//! 解析器共享的可用工具快照：当前定义的工具名与参数名，两者均为 O(1)
//! 查询。

use std::collections::{HashMap, HashSet};

use crate::tools::ToolKind;
use crate::tools::all_tools;

/// Snapshot of the available tools and their defined param names. Parsers
/// accept only tools/params present here and discard everything else.
/// Cheap to build (at most the five built-in tools) and shared through an
/// `Arc` by the registry cache.
/// 可用工具及其已定义参数名的快照。解析器只接受其中出现的工具与参数，
/// 其余内容一律丢弃。构建开销低（最多五个内置工具），由注册表缓存以
/// `Arc` 共享。
#[derive(Debug, Clone)]
pub struct EnabledToolSet {
    /// Tool name → kind, for O(1) "is this a defined tool name" checks.
    /// 工具名 → 工具类型，用于 O(1) 判定字符串是否为已定义工具名。
    by_name: HashMap<&'static str, ToolKind>,
    /// Tool name → set of its defined param names.
    /// 工具名 → 其已定义参数名集合。
    params: HashMap<&'static str, HashSet<&'static str>>,
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
        Self { by_name, params }
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

    /// Whether `name` is a defined tool in the set.
    /// `name` 是否为集合中已定义的工具。
    pub fn contains_tool(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Resolve `name` to its tool kind, if it is in the set.
    /// 若 `name` 在集合中，解析为对应的工具类型。
    pub fn tool_kind(&self, name: &str) -> Option<ToolKind> {
        self.by_name.get(name).copied()
    }

    /// Whether `param` is a defined param of tool `tool_name`.
    /// `param` 是否为工具 `tool_name` 的已定义参数。
    pub fn contains_param(&self, tool_name: &str, param: &str) -> bool {
        self.params
            .get(tool_name)
            .is_some_and(|names| names.contains(param))
    }

    /// Canonical-order (the `all_tools` order) names of the tools in the
    /// set — used by the registry as the cache fingerprint.
    /// 集合内工具按规范顺序（`all_tools` 顺序）的名称——注册表用作缓存
    /// 指纹。
    pub fn tool_names(&self) -> Vec<String> {
        all_tools()
            .iter()
            .filter(|kind| self.contains_tool(kind.name()))
            .map(|kind| kind.name().to_string())
            .collect()
    }
}

impl Default for EnabledToolSet {
    fn default() -> Self {
        Self::all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_names_resolves_and_drops_unknown() {
        let set = EnabledToolSet::from_names(&["read".to_string(), "bogus".to_string()]);
        assert!(set.contains_tool("read"));
        assert!(!set.contains_tool("bogus"));
        assert!(!set.contains_tool("shell"));
    }

    #[test]
    fn from_tool_kinds_builds_param_index() {
        let set = EnabledToolSet::from_tool_kinds(&[ToolKind::Edit]);
        assert!(set.contains_tool("edit"));
        assert!(set.contains_param("edit", "file_path"));
        assert!(set.contains_param("edit", "old_string"));
        assert!(set.contains_param("edit", "new_string"));
        assert!(set.contains_param("edit", "replace_all"));
        assert!(!set.contains_param("edit", "command"));
    }

    #[test]
    fn default_equals_all() {
        let set = EnabledToolSet::default();
        assert_eq!(set.tool_names().len(), all_tools().len());
    }

    #[test]
    fn contains_tool_and_param_lookups() {
        let set = EnabledToolSet::all();
        assert_eq!(set.tool_kind("read"), Some(ToolKind::Read));
        assert_eq!(set.tool_kind("nonsense"), None);
        assert!(set.contains_param("read", "file_path"));
        // 参数名区分大小写，与标签名一致。
        assert!(!set.contains_param("read", "FILE_PATH"));
    }

    #[test]
    fn tool_names_returns_canonical_order() {
        // 乱序输入也会按 all_tools 顺序规范化，保证指纹稳定。
        let set = EnabledToolSet::from_names(&[
            "skill".to_string(),
            "read".to_string(),
            "write".to_string(),
        ]);
        assert_eq!(set.tool_names(), vec!["read", "write", "skill"]);
    }
}
