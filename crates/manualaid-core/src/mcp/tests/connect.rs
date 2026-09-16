//! Unit tests for the MCP connection layer's result mapping.
//! MCP 连接层结果映射的单元测试。
//!
//! # Scope
//! Only the mapping from a server result onto [`ToolResult`] is exercised
//! here. Spawning a child process and the MCP handshake need a real server,
//! which the integration tests provide; driving them from a unit test would
//! make the suite depend on the host's process table.
//! # 范围
//! 这里只检验从服务器结果到 [`ToolResult`] 的映射。启动子进程与 MCP 握手
//! 需要真实服务器，由集成测试提供；在单元测试中驱动它们会让测试套件依赖
//! 宿主机的进程表。

use rmcp::model::{
    AudioContent, CallToolResult, ContentBlock, ImageContent, ResourceContents, TextContent,
};

use crate::mcp::connect::to_tool_result;

/// One text content block carrying `text`.
/// 一个携带 `text` 的文本内容块。
fn text(text: &str) -> ContentBlock {
    ContentBlock::Text(TextContent::new(text))
}

#[test]
fn text_content_is_passed_through_verbatim() {
    let result = to_tool_result("mcp_s_t", CallToolResult::success(vec![text("hello")]));

    assert!(result.success);
    assert_eq!(result.output, "hello");
    assert_eq!(result.tool_name, "mcp_s_t");
}

#[test]
fn a_result_the_server_marked_as_an_error_becomes_a_failure() {
    let result = to_tool_result("mcp_s_t", CallToolResult::error(vec![text("boom")]));

    assert!(!result.success);
    assert_eq!(result.output, "boom");
}

#[test]
fn binary_blocks_become_markers_naming_what_was_dropped() {
    let blocks = vec![
        ContentBlock::Image(ImageContent::new("AAAA", "image/png")),
        ContentBlock::Audio(AudioContent::new("BBBB", "audio/wav")),
    ];

    let result = to_tool_result("mcp_s_t", CallToolResult::success(blocks));

    assert_eq!(result.output, "[image: image/png]\n[audio: audio/wav]");
}

#[test]
fn an_embedded_text_resource_contributes_its_text() {
    let block = ContentBlock::resource(ResourceContents::text("body", "file:///a.txt"));

    let result = to_tool_result("mcp_s_t", CallToolResult::success(vec![block]));

    assert_eq!(result.output, "body");
}

#[test]
fn multiple_blocks_are_joined_by_newlines() {
    let blocks = vec![text("first"), text("second")];

    let result = to_tool_result("mcp_s_t", CallToolResult::success(blocks));

    assert_eq!(result.output, "first\nsecond");
}

#[test]
fn an_empty_result_yields_an_empty_body() {
    let result = to_tool_result("mcp_s_t", CallToolResult::success(Vec::new()));

    assert!(result.success);
    assert_eq!(result.output, "");
}
