//! Integration tests for the async file I/O helpers.
//! 异步文件 I/O 辅助函数的集成测试。

use std::sync::atomic::{AtomicUsize, Ordering};

use manualaid_core::async_fs::{read_file, write_file};

fn temp_path(tag: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "manualaid-core-async-fs-{}-{}-{tag}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ))
}

#[tokio::test]
async fn write_read_round_trip() {
    let path = temp_path("roundtrip");
    write_file(&path, "data").await.unwrap();
    assert_eq!(read_file(&path).await.unwrap(), "data");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn write_creates_parent_dirs() {
    let dir = std::env::temp_dir().join(format!("manualaid-core-async-dir-{}", std::process::id()));
    let path = dir.join("a").join("b.txt");
    write_file(&path, "x").await.unwrap();
    assert!(path.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn read_missing_file_is_an_error() {
    let error = read_file("Z:/definitely-missing/manualaid.txt")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cannot read file"));
}

#[tokio::test]
async fn write_through_existing_file_fails() {
    let path = temp_path("parent-file");
    std::fs::write(&path, "file").unwrap();
    let error = write_file(path.join("child.txt"), "x").await.unwrap_err();
    assert!(error.to_string().contains("cannot create parent directory"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn write_to_directory_path_fails() {
    let dir = std::env::temp_dir().join(format!(
        "manualaid-core-async-dir-write-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let error = write_file(&dir, "x").await.unwrap_err();
    assert!(error.to_string().contains("cannot write file"));
    let _ = std::fs::remove_dir_all(&dir);
}
