use super::*;

#[test]
fn exempt_path_matches_descendants_only() {
    let root = std::env::temp_dir();
    let exempt = root.join("manualaid-exempt");
    std::fs::create_dir_all(&exempt).expect("create exempt dir");
    assert!(is_exempt_path(
        &exempt.join("x.txt"),
        std::slice::from_ref(&exempt)
    ));
    assert!(!is_exempt_path(&root.join("other.txt"), &[exempt]));
}

#[cfg(windows)]
#[test]
fn exempt_path_matches_ignoring_ascii_case_on_windows() {
    let root = std::env::temp_dir();
    let exempt = root.join("manualaid-exempt-ci");
    std::fs::create_dir_all(&exempt).expect("create exempt dir");
    let variant = PathBuf::from(exempt.to_string_lossy().to_lowercase());
    assert!(is_exempt_path(
        &variant.join("x.txt"),
        std::slice::from_ref(&exempt)
    ));
}
