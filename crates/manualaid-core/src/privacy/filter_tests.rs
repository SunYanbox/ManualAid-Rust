use super::*;
use crate::privacy::PrivacyMaskExtension;

fn ext(regex: &[(&str, &str)], literal: &[(&str, &str)]) -> PrivacyMaskExtension {
    PrivacyMaskExtension {
        regex: regex
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        literal: literal
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

#[test]
fn test_literal_substring_mask() {
    let masker = PrivacyMasker::from_extensions(&ext(&[], &[("UserName", "Alice")])).unwrap();
    let (masked, mapping) = masker.sanitize("C:/User/Alice/Download").unwrap();
    let rest = masked
        .strip_prefix("C:/User/")
        .and_then(|s| s.strip_suffix("/Download"))
        .expect("masked path keeps prefix and suffix");
    assert!(rest.starts_with("[PRV_UserName_"));
    assert!(!masked.contains("Alice"));
    assert_eq!(mapping.get(rest).map(String::as_str), Some("Alice"));
}

#[test]
fn test_anchored_regex_full_text() {
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("ExamApiKey", r"^sk-[A-Za-z0-9]{7}$")], &[]))
            .unwrap();
    let (masked, mapping) = masker.sanitize("sk-114dsx6").unwrap();
    assert!(masked.starts_with("[PRV_ExamApiKey_"));
    assert!(masked.ends_with(']'));
    assert_eq!(mapping.get(&masked).map(String::as_str), Some("sk-114dsx6"));
}

#[test]
fn test_unanchored_regex_substring_in_code() {
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("ExamApiKey", r"sk-[A-Za-z0-9]{7}")], &[])).unwrap();
    let (masked, _) = masker.sanitize(r#"let a = "sk-114dsx6";"#).unwrap();
    assert!(masked.starts_with(r#"let a = "[PRV_ExamApiKey_"#));
    assert!(masked.ends_with(r#"]";"#));
    assert!(!masked.contains("sk-114dsx6"));
}

#[test]
fn test_same_value_same_id_across_calls() {
    let masker = PrivacyMasker::from_extensions(&ext(&[], &[("UserName", "Alice")])).unwrap();
    let (m1, map1) = masker.sanitize("hi Alice").unwrap();
    let (m2, _) = masker.sanitize("hi Alice").unwrap();
    assert_eq!(m1, m2);
    assert_eq!(map1.len(), 1);
    assert!(map1.values().any(|v| v == "Alice"));
}

#[test]
fn test_priority_regex_over_literal_over_cloakrs() {
    // cloakrs finds the email span; the literal `alice` overlaps it;
    // the regex `alice@` overlaps both and must win.
    // cloakrs 检出邮箱区间；精确匹配 `alice` 与其重叠；正则 `alice@`
    // 与两者都重叠且必须胜出。
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("EmailLocal", "alice@")], &[("UserName", "alice")]))
            .unwrap();
    let (masked, mapping) = masker.sanitize("contact alice@example.com").unwrap();
    let rest = masked
        .strip_prefix("contact ")
        .and_then(|s| s.strip_suffix("example.com"))
        .expect("regex match replaced only the overlap winner");
    assert!(rest.starts_with("[PRV_EmailLocal_"));
    assert_eq!(mapping.get(rest).map(String::as_str), Some("alice@"));
}

#[test]
fn test_non_overlapping_multi_source_all_applied() {
    let masker = PrivacyMasker::from_extensions(&ext(
        &[("ExamApiKey", r"sk-[A-Za-z0-9]{7}")],
        &[("UserName", "Carol")],
    ))
    .unwrap();
    let (masked, mapping) = masker
        .sanitize("Carol lives at alice@example.com, key sk-2468135")
        .unwrap();
    assert!(!masked.contains("Carol"));
    assert!(!masked.contains("alice@example.com"));
    assert!(!masked.contains("sk-2468135"));
    assert!(mapping.values().any(|v| v == "Carol"));
    assert!(mapping.values().any(|v| v == "alice@example.com"));
    assert!(mapping.values().any(|v| v == "sk-2468135"));
    assert_eq!(
        restore_masked_data(&masked, &mapping),
        "Carol lives at alice@example.com, key sk-2468135"
    );
}

#[test]
fn test_invalid_regex_is_ignored() {
    let masker = PrivacyMasker::from_extensions(&ext(
        &[("Bad", "([unclosed"), ("Good", r"sk-[0-9]{7}")],
        &[],
    ))
    .unwrap();
    let (masked, mapping) = masker.sanitize("sk-9991111").unwrap();
    assert!(masked.starts_with("[PRV_Good_"));
    assert!(!masked.contains("[PRV_Bad_"));
    assert_eq!(mapping.get(&masked).map(String::as_str), Some("sk-9991111"));
}

#[test]
fn test_empty_match_regex_is_ignored() {
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("Empty", "a*"), ("Good", "aa")], &[])).unwrap();
    let (masked, mapping) = masker.sanitize("aaa").unwrap();
    assert!(masked.starts_with("[PRV_Good_"));
    assert!(masked.ends_with('a'));
    assert_eq!(restore_masked_data(&masked, &mapping), "aaa");
}

#[test]
fn test_restore_roundtrip() {
    let masker = PrivacyMasker::from_extensions(&ext(
        &[("ExamApiKey", r"sk-[A-Za-z0-9]{7}")],
        &[("UserName", "Alice")],
    ))
    .unwrap();
    let text = "Alice: sk-1234567";
    let (masked, mapping) = masker.sanitize(text).unwrap();
    assert_eq!(restore_masked_data(&masked, &mapping), text);
}

#[test]
fn test_restore_distinguishes_suffix_numbers() {
    let mapping = HashMap::from([
        ("[PRV_EMAIL_1]".to_string(), "a@b.com".to_string()),
        ("[PRV_EMAIL_10]".to_string(), "j@k.com".to_string()),
    ]);
    assert_eq!(
        restore_masked_data("x [PRV_EMAIL_10] y [PRV_EMAIL_1]", &mapping),
        "x j@k.com y a@b.com"
    );
}

#[test]
fn test_restore_keeps_unknown_and_brackets() {
    let mapping = HashMap::new();
    assert_eq!(
        restore_masked_data("a [PRV_UNKNOWN_1] b [c", &mapping),
        "a [PRV_UNKNOWN_1] b [c"
    );
}

#[test]
fn test_sanitize_prompt_builtin_only() {
    let (masked, mapping) = sanitize_prompt("contact jane@example.com").unwrap();
    assert!(masked.starts_with("contact [PRV_EMAIL_"));
    assert!(masked.ends_with(']'));
    assert!(mapping.values().any(|v| v == "jane@example.com"));
    assert_eq!(
        restore_masked_data(&masked, &mapping),
        "contact jane@example.com"
    );
}

#[test]
fn test_combined_regex_multiple_patterns() {
    // Use plaintexts unique to this test: the process-wide registry maps a
    // plaintext to one stable ID, so reusing `"aa"`/`"bb"` from other tests
    // would make the mask-ID prefix depend on test scheduling.
    // 使用本测试独有的明文：进程级注册表对同一明文只保留一个稳定 ID，
    // 若与其他测试共用 `"aa"`/`"bb"`，掩码 ID 前缀会受测试调度顺序影响。
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("A", "Alpha"), ("B", "Beta")], &[])).unwrap();
    let (masked, mapping) = masker.sanitize("Alpha Beta").unwrap();
    assert!(masked.starts_with("[PRV_A_"));
    assert!(masked.contains("[PRV_B_"));
    assert_eq!(restore_masked_data(&masked, &mapping), "Alpha Beta");
}

#[test]
fn test_combined_regex_fallback_on_duplicate_named_group() {
    // Duplicate capture-group names make the combined regex unbuildable, so
    // the tier must fall back to per-pattern scanning.
    // 重复的捕获组名会使合并正则构建失败，该层应回退到逐模式扫描。
    let masker =
        PrivacyMasker::from_extensions(&ext(&[("A", r"(?P<dup>ab)"), ("B", r"(?P<dup>cd)")], &[]))
            .unwrap();
    let (masked, mapping) = masker.sanitize("ab cd").unwrap();
    assert!(!masked.contains("ab"));
    assert!(!masked.contains("cd"));
    assert_eq!(restore_masked_data(&masked, &mapping), "ab cd");
}

#[test]
fn test_overlaps_ignores_degenerate_intervals() {
    // An interval with end <= start can never overlap an occupied span;
    // the guard must short-circuit before any index arithmetic.
    // end <= start 的退化区间不可能与已占用区间重叠；该保护必须
    // 在任何下标计算之前短路。
    let mut set = IntervalSet::default();
    set.insert(3, 10);
    assert!(!set.overlaps(5, 5));
    assert!(!set.overlaps(5, 3));
    assert!(set.overlaps(5, 9));
}
