use indexmap::IndexMap;

use super::*;

#[test]
fn installed_tools_carry_their_exposed_names() {
    with_store_lock(|| {
        reset();
        install_one(stdio("fs"), vec![tool("fs", "read"), tool("fs", "write")]);
        let names: Vec<String> = all_tools()
            .into_iter()
            .map(|entry| entry.exposed_name)
            .collect();
        assert_eq!(names, ["mcp_fs_read", "mcp_fs_write"]);
        reset();
    });
}

#[test]
fn a_disabled_server_contributes_no_usable_tool() {
    with_store_lock(|| {
        reset();
        let mut server = stdio("fs");
        server.enabled = false;
        install_one(server, vec![tool("fs", "read")]);

        // The discovered tools stay visible for the management menu, but
        // nothing advertises or resolves them while the server is off.
        // 已发现的工具仍对管理菜单可见，但服务器关闭时不会被展示或解析。
        assert_eq!(all_tools().len(), 1);
        assert!(enabled_tools().is_empty());
        assert!(resolve_tool("mcp_fs_read").is_none());
        reset();
    });
}

#[test]
fn enabled_servers_are_the_only_ones_offered() {
    with_store_lock(|| {
        reset();
        let mut off = stdio("off");
        off.enabled = false;
        install_many(vec![
            state(stdio("on"), vec![tool("on", "read")]),
            state(off, vec![tool("off", "read")]),
        ]);
        let names: Vec<String> = enabled_tools()
            .into_iter()
            .map(|entry| entry.exposed_name)
            .collect();
        assert_eq!(names, ["mcp_on_read"]);
        reset();
    });
}

#[test]
fn resolve_tool_matches_the_whole_exposed_name() {
    with_store_lock(|| {
        reset();
        install_one(stdio("fs"), vec![tool("fs", "read_file")]);
        assert!(resolve_tool("mcp_fs_read_file").is_some());
        // Partial and bare names are not resolved: the name is matched whole
        // rather than split back into its parts.
        // 部分名称与裸名称都不会被解析：名称按整体匹配，而非反解为各部分。
        assert!(resolve_tool("read_file").is_none());
        assert!(resolve_tool("mcp_fs_read").is_none());
        assert!(resolve_tool("mcp_other_read_file").is_none());
        reset();
    });
}

#[test]
fn the_first_server_keeps_a_duplicated_exposed_name() {
    // `mcp_a_b_c` is ambiguous between server `a` with tool `b_c` and server
    // `a_b` with tool `c`; the earlier server keeps it so resolution stays
    // deterministic.
    // `mcp_a_b_c` 在服务器 `a` 的工具 `b_c` 与服务器 `a_b` 的工具 `c` 之间
    // 存在歧义；靠前的服务器保留该名称，使解析保持确定。
    with_store_lock(|| {
        reset();
        install_many(vec![
            state(stdio("a"), vec![tool("a", "b_c")]),
            state(stdio("a_b"), vec![tool("a_b", "c")]),
        ]);
        let entries = all_tools();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server_name, "a");
        assert_eq!(entries[0].tool_name, "b_c");
        reset();
    });
}

#[test]
fn server_status_reports_each_configured_server() {
    with_store_lock(|| {
        reset();
        let mut broken = stdio("broken");
        broken.command = None;
        install_many(vec![
            state(stdio("fs"), vec![tool("fs", "read")]),
            ServerState {
                config: broken,
                tools: Vec::new(),
                error: Some("no command".to_string()),
                client: None,
            },
        ]);

        let status = server_status();
        assert_eq!(status.len(), 2);
        assert_eq!(status[0].name, "fs");
        assert_eq!(status[0].transport, McpTransportKind::Stdio);
        assert!(status[0].enabled);
        assert_eq!(status[0].tool_count, 1);
        assert_eq!(status[0].error, None);
        assert_eq!(status[1].name, "broken");
        assert_eq!(status[1].tool_count, 0);
        assert_eq!(status[1].error.as_deref(), Some("no command"));
        reset();
    });
}

#[test]
fn reset_clears_every_server() {
    with_store_lock(|| {
        install_one(stdio("fs"), vec![tool("fs", "read")]);
        assert!(!all_tools().is_empty());
        reset();
        assert!(all_tools().is_empty());
        assert!(server_status().is_empty());
        assert!(resolve_tool("mcp_fs_read").is_none());
    });
}

#[test]
fn call_tool_reports_an_unknown_exposed_name() {
    with_store_lock(|| {
        reset();
        let result = block_on(call_tool("mcp_absent_tool", &IndexMap::new()));
        assert!(!result.success);
        assert!(result.is_fallback);
        assert_eq!(result.tool_name, "mcp_absent_tool");
        reset();
    });
}

#[test]
fn call_tool_reports_a_server_that_is_not_connected() {
    with_store_lock(|| {
        reset();
        install_one(stdio("fs"), vec![tool("fs", "read")]);
        let result = block_on(call_tool("mcp_fs_read", &IndexMap::new()));
        assert!(!result.success);
        assert_eq!(result.tool_name, "mcp_fs_read");
        reset();
    });
}

#[test]
fn connect_all_installs_every_declared_server() {
    with_store_lock(|| {
        reset();
        let mut broken = stdio("broken");
        broken.command = None;
        let mut disabled = stdio("disabled");
        disabled.enabled = false;
        block_on(connect_all(&[broken, disabled, stdio("good")])).expect("connect_all succeeds");

        // Every declared server keeps its slot, and the three ways a server
        // can contribute nothing stay distinguishable: `broken` fails
        // validation, `disabled` is never attempted, and `good` names a
        // command that does not exist, so the connection itself fails.
        // 每个已声明服务器都保留自己的位置，且三种「未贡献工具」的情形保持可
        // 区分：`broken` 校验失败，`disabled` 从不尝试连接，`good` 指向一个
        // 不存在的命令，因此连接本身失败。
        let status = server_status();
        assert_eq!(status.len(), 3);
        assert!(status[0].error.is_some());
        assert!(!status[1].enabled);
        assert_eq!(status[1].error, None);
        assert!(status[2].error.is_some());
        assert!(all_tools().is_empty());
        assert!(enabled_tools().is_empty());
        reset();
    });
}
