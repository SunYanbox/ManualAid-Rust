use super::*;

#[test]
fn extracts_fenced_block() {
    let blocks = extract_json_blocks("prefix\n```json\n{\"key\": \"val\"}\n```\nsuffix");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
}

#[test]
fn extracts_crlf_fence() {
    let blocks = extract_json_blocks("prefix\r\n```json\r\n{\"key\": \"val\"}\r\n```\r\n");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
}

#[test]
fn extracts_func_calls_fence() {
    let blocks = extract_json_blocks("prefix\n```func_calls\n{\"key\": \"val\"}\n```\nsuffix");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
}

#[test]
fn extracts_func_calls_crlf_fence() {
    let blocks = extract_json_blocks("prefix\r\n```func_calls\r\n{\"key\": \"val\"}\r\n```\r\n");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "{\"key\": \"val\"}");
}

#[test]
fn bare_fence_skips_non_json() {
    let blocks = extract_json_blocks("```\nsome text\n```\n```json\n{\"a\": 1}\n```");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].0, "{\"a\": 1}");
}

#[test]
fn inline_json_with_tool_use_is_detected() {
    let blocks = extract_json_blocks("{\"tool_use\": \"read\", \"params\": {}}");
    assert_eq!(blocks.len(), 1);
}
