use super::*;

#[test]
fn within_workspace_matches_nested_paths() {
    let root = std::env::temp_dir();
    assert!(is_within_workspace(&root.join("a.txt"), &root));
    assert!(is_within_workspace(&root.join("sub"), &root));
    assert!(!is_within_workspace(
        &root.parent().unwrap().join("other"),
        &root
    ));
}

#[test]
fn within_workspace_handles_nonexistent_target() {
    let root = std::env::temp_dir();
    let target = root.join("manualaid-ws-nonexistent").join("new.txt");
    assert!(is_within_workspace(&target, &root));
}

#[cfg(windows)]
#[test]
fn within_workspace_resolves_nonexistent_target_through_junction() {
    use std::os::windows::fs::symlink_dir;
    let root = std::env::temp_dir().join(format!("manualaid-ws-junction-{}", std::process::id()));
    let real = root.join("real");
    let link = root.join("link");
    std::fs::create_dir_all(&real).expect("create real dir");
    // Directory symlink creation needs Developer Mode or an elevated
    // shell; skip when the OS denies it so the suite still runs on
    // unprivileged machines.
    // 创建目录符号链接需要开发者模式或提权终端；操作系统拒绝时跳过，
    // 保证测试套件在未提权机器上仍可运行。
    if symlink_dir(&real, &link).is_err() {
        return;
    }
    let target = link.join("new.txt");
    assert!(is_within_workspace(&target, &link));
    let _ = std::fs::remove_dir(&link);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(not(windows))]
#[test]
fn within_workspace_resolves_nonexistent_target_through_symlink() {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("manualaid-ws-symlink-{}", std::process::id()));
    let real = root.join("real");
    let link = root.join("link");
    std::fs::create_dir_all(&real).expect("create real dir");
    symlink(&real, &link).expect("create symlink");
    let target = link.join("new.txt");
    assert!(is_within_workspace(&target, &link));
    let _ = std::fs::remove_dir(&link);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(windows)]
#[test]
fn within_workspace_ignores_ascii_case_on_windows() {
    let root = std::env::temp_dir();
    let target = root.join("manualaid-ws-case").join("new.txt");
    let variant = PathBuf::from(target.to_string_lossy().to_lowercase());
    assert!(is_within_workspace(&variant, &root));
}
