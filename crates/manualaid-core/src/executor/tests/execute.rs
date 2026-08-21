use super::*;

#[test]
fn tool_result_constructors_zero_stats_fields() {
    let ok = ToolResult::success("read", "out", true);
    assert_eq!(ok.execution_duration_ms, 0);
    assert_eq!(ok.estimated_tokens, 0);
    let err = ToolResult::failure("edit", "boom");
    assert_eq!(err.execution_duration_ms, 0);
    assert_eq!(err.estimated_tokens, 0);
}

#[tokio::test]
async fn execute_records_execution_duration() {
    let root = std::env::temp_dir().join(format!("manualaid-exec-dur-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("target.txt");
    std::fs::write(&file, "hello").unwrap();
    let executor = Executor::new(Auditor::new(root.clone()), Arc::new(None));
    let call = crate::parser::FormatRegistry::new()
        .parse(&format!(
            "<read><file_path>{}</file_path></read>",
            file.display()
        ))
        .unwrap()
        .calls
        .remove(0);
    let start = std::time::Instant::now();
    let result = executor.execute(call).await;
    assert!(result.success);
    // Truncated milliseconds never exceed the wall-clock elapsed since
    // the call started, so the recorded value stays consistent.
    // 截断后的毫秒数不会超过调用开始以来的墙钟耗时。
    assert!(result.execution_duration_ms <= start.elapsed().as_millis() as u64);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn execute_failed_paths_keep_zero_duration() {
    let executor = Executor::new(Auditor::new(std::env::temp_dir()), Arc::new(None));
    // 未知工具名会被解析器丢弃，此处直接构造调用以覆盖执行器的
    // unknown-tool 守卫分支。
    let call = crate::parser::ParsedToolCall {
        tool_name: "nonsense".to_string(),
        params: IndexMap::new(),
        format: crate::tools::ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    };
    let result = executor.execute(call).await;
    assert!(!result.success);
    assert_eq!(result.execution_duration_ms, 0);
}

#[tokio::test]
async fn unclosed_flags_fail_before_execution() {
    let executor = Executor::new(Auditor::new(std::env::temp_dir()), Arc::new(None));
    let base = crate::parser::ParsedToolCall {
        tool_name: "read".to_string(),
        params: IndexMap::new(),
        format: crate::tools::ToolCallFormat::Xml,
        source_offset: None,
        unclosed_param: false,
        unclosed_tool: false,
    };

    let cases = [
        (
            true,
            false,
            "Unclosed tool call `read` (missing closing tag)",
        ),
        (false, true, "Unclosed parameter in tool call `read`"),
        (
            true,
            true,
            "Unclosed tool call `read` and an unclosed parameter",
        ),
    ];
    for (unclosed_tool, unclosed_param, expected) in cases {
        let call = crate::parser::ParsedToolCall {
            unclosed_tool,
            unclosed_param,
            ..base.clone()
        };
        let pre = executor.pre_check(&call).await;
        assert!(pre.is_some());
        let result = executor.execute(call).await;
        assert!(!result.success);
        assert!(result.output.contains(expected), "{}", result.output);
    }
}
