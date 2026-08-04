use super::*;

fn hex_of(text: &str) -> String {
    hash_to_hex(&hash_plaintext(text))
}

#[test]
fn test_hash_plaintext_consistency() {
    assert_eq!(
        hash_plaintext("jane@example.com"),
        hash_plaintext("jane@example.com")
    );
}

#[test]
fn test_hash_plaintext_different() {
    assert_ne!(hash_plaintext("a@b.com"), hash_plaintext("c@d.com"));
}

#[test]
fn test_hash_length() {
    assert_eq!(hash_plaintext("test").len(), 64);
}

#[test]
fn test_get_or_create_stable_and_counters() {
    let reg = PrivacyRegistry::new();
    let a = reg.get_or_create("EMAIL", "jane@example.com").unwrap();
    let b = reg.get_or_create("EMAIL", "jane@example.com").unwrap();
    assert_eq!(a, "[PRV_EMAIL_1]");
    assert_eq!(a, b);

    let c = reg.get_or_create("EMAIL", "bob@example.com").unwrap();
    assert_eq!(c, "[PRV_EMAIL_2]");

    let d = reg.get_or_create("PHONE", "13800138000").unwrap();
    assert_eq!(d, "[PRV_PHONE_1]");

    assert_eq!(reg.len().unwrap(), 3);
    assert!(!reg.is_empty().unwrap());
    assert!(reg.contains("jane@example.com").unwrap());
    assert!(!reg.contains("nobody@example.com").unwrap());
}

#[test]
fn test_snapshot_roundtrip() {
    let reg = PrivacyRegistry::new();
    let id1 = reg.get_or_create("EMAIL", "jane@example.com").unwrap();
    let id2 = reg.get_or_create("PHONE", "13800138000").unwrap();

    let snap = reg.export_snapshot().unwrap();
    assert_eq!(snap.entries.len(), 2);
    // Sorted by mask_id.
    assert_eq!(snap.entries[0].mask_id, "[PRV_EMAIL_1]");
    assert_eq!(snap.entries[1].mask_id, "[PRV_PHONE_1]");
    assert_eq!(snap.entries[0].hash_hex.len(), 128);

    let reg2 = PrivacyRegistry::from_snapshot(snap).unwrap();
    assert_eq!(
        reg2.get_or_create("EMAIL", "jane@example.com").unwrap(),
        id1
    );
    assert_eq!(reg2.get_or_create("PHONE", "13800138000").unwrap(), id2);
    assert_eq!(
        reg2.get_or_create("EMAIL", "other@example.com").unwrap(),
        "[PRV_EMAIL_2]"
    );
    assert_eq!(reg2.len().unwrap(), 3);
}

#[test]
fn test_snapshot_rejects_invalid_hex() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "[PRV_EMAIL_1]".to_string(),
            hash_hex: "xyz".to_string(),
        }],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Parse(_))
    ));
}

#[test]
fn test_snapshot_rejects_duplicate_mask_id() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![
            PrivacyRegistryEntry {
                mask_id: "[PRV_EMAIL_1]".to_string(),
                hash_hex: hex_of("a@b.com"),
            },
            PrivacyRegistryEntry {
                mask_id: "[PRV_EMAIL_1]".to_string(),
                hash_hex: hex_of("c@d.com"),
            },
        ],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_snapshot_rejects_duplicate_hash() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![
            PrivacyRegistryEntry {
                mask_id: "[PRV_EMAIL_1]".to_string(),
                hash_hex: hex_of("a@b.com"),
            },
            PrivacyRegistryEntry {
                mask_id: "[PRV_PHONE_1]".to_string(),
                hash_hex: hex_of("a@b.com"),
            },
        ],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_snapshot_rejects_unparseable_mask_id() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "PRV_EMAIL_1".to_string(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_snapshot_counter_too_small_is_rejected() {
    let mut counters = HashMap::new();
    counters.insert("EMAIL".to_string(), 1);
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "[PRV_EMAIL_2]".to_string(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters,
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_snapshot_counter_larger_than_max_is_allowed() {
    let mut counters = HashMap::new();
    counters.insert("EMAIL".to_string(), 3);
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "[PRV_EMAIL_2]".to_string(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters,
    };
    let reg = PrivacyRegistry::from_snapshot(snap).unwrap();
    assert_eq!(
        reg.get_or_create("EMAIL", "new@example.com").unwrap(),
        "[PRV_EMAIL_4]"
    );
}

#[test]
fn test_snapshot_accepts_legacy_mask_id() {
    let mut counters = HashMap::new();
    counters.insert("EMAIL".to_string(), 2);
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "[EMAIL_2]".to_string(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters,
    };
    let reg = PrivacyRegistry::from_snapshot(snap).unwrap();
    assert!(reg.contains("a@b.com").unwrap());
    assert_eq!(
        reg.get_or_create("EMAIL", "next@example.com").unwrap(),
        "[PRV_EMAIL_3]"
    );
}

#[test]
fn test_extract_entity_type() {
    assert_eq!(extract_entity_type("[PRV_EMAIL_1]"), "EMAIL");
    assert_eq!(extract_entity_type("[PRV_PHONE_42]"), "PHONE");
    assert_eq!(extract_entity_type("[PRV_CREDIT_CARD_3]"), "CREDIT_CARD");
    assert_eq!(extract_entity_type("[PRV_API_KEY_1]"), "API_KEY");
    assert_eq!(extract_entity_type("[EMAIL_2]"), "EMAIL");
}

#[test]
fn test_normalize_maps_to_stable_ids() {
    let reg = PrivacyRegistry::new();
    let (stable, mapping) = reg
        .normalize(
            "use [EMAIL_1]",
            &[("[EMAIL_1]".to_string(), "a@b.com".to_string())],
        )
        .unwrap();
    assert_eq!(stable, "use [PRV_EMAIL_1]");
    assert_eq!(
        mapping.get("[PRV_EMAIL_1]").map(String::as_str),
        Some("a@b.com")
    );
}

#[test]
fn test_normalize_avoids_chained_replacement() {
    let reg = PrivacyRegistry::new();
    let (stable, mapping) = reg
        .normalize(
            "x [PRV_EMAIL_10] y [PRV_EMAIL_1]",
            &[
                ("[PRV_EMAIL_10]".to_string(), "a@b.com".to_string()),
                ("[PRV_EMAIL_1]".to_string(), "c@d.com".to_string()),
            ],
        )
        .unwrap();
    // `a@b.com` gets the fresh ID `[PRV_EMAIL_1]`; replacing `[PRV_EMAIL_10]`
    // must not be re-matched by the later `[PRV_EMAIL_1]` -> `[PRV_EMAIL_2]`
    // replacement.
    // `a@b.com` 在全新注册表中获得 `[PRV_EMAIL_1]`；`[PRV_EMAIL_10]` 被替换后
    // 不能被后续 `[PRV_EMAIL_1]` -> `[PRV_EMAIL_2]` 再次匹配。
    assert_eq!(stable, "x [PRV_EMAIL_1] y [PRV_EMAIL_2]");
    assert_eq!(
        mapping.get("[PRV_EMAIL_1]").map(String::as_str),
        Some("a@b.com")
    );
    assert_eq!(
        mapping.get("[PRV_EMAIL_2]").map(String::as_str),
        Some("c@d.com")
    );
}

#[test]
fn test_normalize_longest_placeholder_matches_first() {
    let reg = PrivacyRegistry::new();
    let (stable, mapping) = reg
        .normalize(
            "[PRV_EMAIL_10] and [PRV_EMAIL_1]",
            &[
                ("[PRV_EMAIL_10]".to_string(), "a@b.com".to_string()),
                ("[PRV_EMAIL_1]".to_string(), "c@d.com".to_string()),
            ],
        )
        .unwrap();
    // `[PRV_EMAIL_1]` is a prefix of `[PRV_EMAIL_10]`; the longer placeholder
    // must win at the same start position.
    // `[PRV_EMAIL_1]` 是 `[PRV_EMAIL_10]` 的前缀；同一起点必须优先匹配更长占位符。
    assert_eq!(stable, "[PRV_EMAIL_1] and [PRV_EMAIL_2]");
    assert_eq!(
        mapping.get("[PRV_EMAIL_1]").map(String::as_str),
        Some("a@b.com")
    );
    assert_eq!(
        mapping.get("[PRV_EMAIL_2]").map(String::as_str),
        Some("c@d.com")
    );
}

#[test]
fn test_normalize_non_bracket_placeholder() {
    let reg = PrivacyRegistry::new();
    let (stable, mapping) = reg
        .normalize(
            "use EMAIL_1",
            &[("EMAIL_1".to_string(), "a@b.com".to_string())],
        )
        .unwrap();
    assert_eq!(stable, "use [PRV_EMAIL_1]");
    assert_eq!(
        mapping.get("[PRV_EMAIL_1]").map(String::as_str),
        Some("a@b.com")
    );
}

#[test]
fn test_snapshot_rejects_empty_mask_id() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: String::new(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_snapshot_rejects_zero_number() {
    let snap = PrivacyRegistrySnapshot {
        entries: vec![PrivacyRegistryEntry {
            mask_id: "[PRV_EMAIL_0]".to_string(),
            hash_hex: hex_of("a@b.com"),
        }],
        counters: HashMap::new(),
    };
    assert!(matches!(
        PrivacyRegistry::from_snapshot(snap),
        Err(CoreError::Config(_))
    ));
}

#[test]
fn test_default_impl_is_empty() {
    let reg = PrivacyRegistry::default();
    assert!(reg.is_empty().unwrap());
    assert_eq!(reg.len().unwrap(), 0);
}

#[test]
fn test_hex_to_hash_case_insensitive_and_invalid() {
    let lower = "ab".repeat(64);
    let upper = "AB".repeat(64);
    assert!(hex_to_hash(&lower).is_some());
    assert_eq!(hex_to_hash(&upper), hex_to_hash(&lower));

    let mut invalid = "0".repeat(128);
    invalid.replace_range(0..1, "G");
    assert!(hex_to_hash(&invalid).is_none());
}

#[test]
fn test_extract_entity_type_without_underscore() {
    assert_eq!(extract_entity_type("[abc]"), "abc");
    assert_eq!(extract_entity_type("plain"), "plain");
}
