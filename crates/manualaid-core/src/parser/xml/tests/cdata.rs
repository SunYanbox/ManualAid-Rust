use super::*;

#[test]
fn cdata_in_param_kept_raw() {
    let outcome = parse(
        "<edit><old_string><![CDATA[<edit> & <read>]]></old_string><new_string>x</new_string></edit>",
    );
    assert_eq!(
        outcome.calls[0]
            .params
            .get("old_string")
            .and_then(Value::as_str),
        Some("<edit> & <read>")
    );
}

#[test]
fn cdata_with_embedded_close_tags_keeps_full_content() {
    // 用户场景：CDATA 内容里含 `<![CDATA[...]]>` 提示与 `<content>`/
    // `</content>` 模板示例时，仍完整保留到真正的 `]]></content>`。
    let outcome = parse(
        "<write><file_path>/f</file_path><content><![CDATA[# 提示词\nwrap values in <![CDATA[...]]>\n<write><content>${content}</content></write>\nEND]]></content></write>",
    );
    let content = outcome.calls[0]
        .params
        .get("content")
        .and_then(Value::as_str)
        .unwrap();
    assert!(content.contains("wrap values in <![CDATA[...]]>"));
    assert!(content.contains("<content>${content}</content>"));
    assert!(content.ends_with("END"));
}

#[test]
fn cdata_outside_param_is_skipped() {
    // 非参数状态的 CDATA 整体跳过，不影响后续解析。
    let outcome = parse("<![CDATA[noise]]><read><file_path>/a</file_path></read>");
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(outcome.calls[0].tool_name, "read");
}

#[test]
fn unterminated_cdata_at_eof_is_ignored() {
    // CDATA 包裹到 EOF 仍未闭合：工具未闭合被整体忽略，参数标签写入
    // 软警告，不 panic。
    let outcome = parse("<read><file_path><![CDATA[x");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<file_path>"));
}

#[test]
fn cdata_mid_value_is_kept_literal() {
    // 参数值中间出现的 `<![CDATA[` 不是包裹，按普通文本原文保留，
    // 参数照常闭合。
    let outcome = parse(
        "<write><file_path>/f</file_path><content>before <![CDATA[x]]> after</content></write>",
    );
    assert_eq!(outcome.calls.len(), 1);
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("before <![CDATA[x]]> after")
    );
}

#[test]
fn cdata_after_leading_whitespace_is_literal() {
    // 参数开始标签后有任何空白（换行/缩进）就不算紧贴，CDATA 标记及
    // 其内容按字面保留为参数值（仅裁首尾换行符）。
    let outcome =
        parse("<write><file_path>/f</file_path><content>\n  <![CDATA[x]]>\n</content></write>");
    assert_eq!(
        outcome.calls[0]
            .params
            .get("content")
            .and_then(Value::as_str),
        Some("  <![CDATA[x]]>")
    );
}

#[test]
fn cdata_close_must_touch_closing_tag() {
    // 严格紧跟：`]]>` 与闭合标签之间有空白就不结束 CDATA，内容原文
    // 累积到 EOF，参数标签视为未正确闭合：工具调用不识别，写入软
    // 警告而不是静默丢弃。
    let outcome =
        parse("<write><file_path>/f</file_path><content><![CDATA[x]]>\n</content></write>");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<content>"));
    let outcome =
        parse("<write><file_path>/f</file_path><content><![CDATA[x]]> </content></write>");
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<content>"));
}

#[test]
fn cdata_wrap_to_eof_warns_unclosed_param() {
    // 文档示例 7：`]]>` 后跟空白再跟 `</file_path>` 不算紧贴闭合标签，
    // CDATA 包裹失败并累积到 EOF，file_path 参数标签未正确闭合：调用
    // 不识别，但写入软警告而非静默丢弃。
    let outcome = parse(
        "<edit>\n  <file_path><![CDATA[README.md]]> </file_path>\n  \
         <old_string>abcdefg,hijklmn</old_string>\n</edit>",
    );
    assert!(outcome.calls.is_empty());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("<file_path>"));
    assert!(outcome.warnings[0].contains("<edit>"));
}

#[test]
fn cdata_close_follows_is_strict() {
    assert!(cdata_close_follows(
        "<content><![CDATA[x]]></content>",
        19,
        "write",
        "content"
    ));
    assert!(cdata_close_follows(
        "<content>x]]></write>",
        10,
        "write",
        "content"
    ));
    assert!(!cdata_close_follows(
        "<content>x]]> </content>",
        10,
        "write",
        "content"
    ));
    assert!(!cdata_close_follows(
        "<content>x]]>\n</content>",
        10,
        "write",
        "content"
    ));
    assert!(!cdata_close_follows(
        "<content>x]]></bogus>",
        10,
        "write",
        "content"
    ));
}

#[test]
fn cdata_close_followed_by_tool_close() {
    // CDATA 后直接跟工具闭合标签：CDATA 正常结束，参数被抛弃并警告，
    // 工具正常闭合。
    let outcome = parse("<edit><old_string><![CDATA[x]]></edit>");
    assert_eq!(outcome.calls.len(), 1);
    assert!(!outcome.calls[0].params.contains_key("old_string"));
    assert_eq!(outcome.warnings.len(), 1);
}
