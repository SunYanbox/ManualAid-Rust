use std::collections::HashMap;
use std::path::Path;

use toml::Table;

use super::*;

#[test]
fn read_enabled_map_missing_file_returns_empty() {
    let dir = temp_dir("missing-config");
    let map = read_enabled_map(&dir.join("config.toml")).expect("missing file is empty");
    assert!(map.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_parses_skill_table() {
    let dir = temp_dir("read-config");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[skill]\n\"/a/b\" = true\n\"/c/d\" = false\n").unwrap();
    let map = read_enabled_map(&path).expect("parse should succeed");
    assert_eq!(map.len(), 2);
    assert!(map["/a/b"]);
    assert!(!map["/c/d"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_non_bool_value_is_config_error() {
    let dir = temp_dir("non-bool");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[skill]\n\"/a/b\" = \"yes\"\n").unwrap();
    assert!(matches!(read_enabled_map(&path), Err(CoreError::Config(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_invalid_toml_is_config_error() {
    let dir = temp_dir("invalid-toml");
    let path = dir.join("config.toml");
    std::fs::write(&path, "not = [valid toml").unwrap();
    assert!(matches!(read_enabled_map(&path), Err(CoreError::Config(_))));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_creates_parent_dirs_and_roundtrips() {
    let dir = temp_dir("write-config");
    let path = dir.join("nested").join("config.toml");
    let mut map = HashMap::new();
    map.insert("C:/Users/alice/.claude/skills/a".to_string(), false);
    write_enabled_map(&path, &map).expect("write should succeed");
    assert!(path.is_file());
    let read = read_enabled_map(&path).expect("read should succeed");
    assert_eq!(read, map);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_preserves_other_sections() {
    let dir = temp_dir("preserve-sections");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[other]\nfoo = 1\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["other"]["foo"], toml::Value::Integer(1));
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_missing_file_seeds_default_template() {
    let dir = temp_dir("seed-template");
    let path = dir.join("config.toml");
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# ManualAid 项目配置文件"));
    assert!(content.contains("[privacy_mask_extension.regex]"));
    assert!(content.contains("[privacy_mask_extension.literal]"));
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_blank_file_seeds_default_template() {
    let dir = temp_dir("seed-blank-template");
    let path = dir.join("config.toml");
    std::fs::write(&path, "  \n\t\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), false);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# ManualAid 项目配置文件"));
    assert_eq!(read_enabled_map(&path).expect("read should succeed"), map);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_preserves_comments_and_removes_stale_entries() {
    let dir = temp_dir("preserve-comments");
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "# 顶部注释\n[other]\nfoo = 1\n\n# 技能注释\n[skill]\n\"old\" = false\n",
    )
    .unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# 顶部注释"));
    assert!(content.contains("# 技能注释"));
    let table: Table = toml::from_str(&content).unwrap();
    assert_eq!(table["other"]["foo"], toml::Value::Integer(1));
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    assert!(!table["skill"].as_table().unwrap().contains_key("old"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_fails_when_parent_is_a_file() {
    let dir = temp_dir("parent-file");
    let blocker = dir.join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    let err = write_enabled_map(&blocker.join("config.toml"), &map).expect_err("write should fail");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn read_enabled_map_when_path_is_a_directory_is_io_error() {
    let dir = temp_dir("read-dir-config");
    let err = read_enabled_map(&dir).expect_err("directory is not a readable config");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_when_path_is_a_directory_is_io_error() {
    let dir = temp_dir("write-dir-config");
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    let err = write_enabled_map(&dir, &map).expect_err("directory is not a writable config");
    assert!(matches!(err, CoreError::Io(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_enabled_map_without_parent_skips_dir_creation() {
    let path = Path::new("manualaid-bare-config-test.toml");
    let _ = std::fs::remove_file(path);
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), false);
    write_enabled_map(path, &map).expect("write should succeed");
    let read = read_enabled_map(path).expect("read should succeed");
    assert_eq!(read, map);
    let _ = std::fs::remove_file(path);
}

#[test]
fn write_enabled_map_replaces_non_table_skill_entry() {
    // A scalar `skill` key must be replaced by a table so the map can be
    // persisted; other keys in the document are untouched.
    // 标量 `skill` 键必须被替换为表才能持久化映射；文档中其余键不受影响。
    let dir = temp_dir("non-table-skill");
    let path = dir.join("config.toml");
    std::fs::write(&path, "skill = \"oops\"\n").unwrap();
    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    write_enabled_map(&path, &map).expect("write should succeed");
    let table: Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(table["skill"]["/a/b"], toml::Value::Boolean(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn write_enabled_map_fails_when_parent_dir_is_not_writable() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("readonly-parent");
    let readonly = dir.join("readonly");
    std::fs::create_dir(&readonly).unwrap();
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555)).unwrap();

    // If the current user can still create entries (e.g. root), this branch
    // is not exercisable; skip rather than fail.
    // 若当前用户仍可创建条目（如 root），此分支无法覆盖；跳过而非失败。
    let probe = readonly.join("probe");
    if std::fs::create_dir(&probe).is_ok() {
        let _ = std::fs::remove_dir_all(&probe);
        std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }

    let mut map = HashMap::new();
    map.insert("/a/b".to_string(), true);
    let path = readonly.join("sub").join("config.toml");
    let err = write_enabled_map(&path, &map).expect_err("write should fail");
    assert!(matches!(err, CoreError::Io(_)));
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
