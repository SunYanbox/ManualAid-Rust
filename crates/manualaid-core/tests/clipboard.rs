use std::sync::{Mutex, MutexGuard};

use manualaid_core::clipboard::{
    ClipboardContent, ClipboardProvider, MockClipboard, inspect_clipboard, read_clipboard,
    write_clipboard,
};

// 剪贴板是系统级共享资源，所有会写入剪贴板的测试（clipboard 模块与
// self_check 全流程测试）必须持有这把锁串行执行，避免并行运行时互相覆盖。
// 当前阶段暂不对剪贴板测试进行 Mock 与还原。
pub(crate) static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

// 当前阶段暂不对剪贴板相关测试进行 Mock 与还原，测试写入的内容会保留在系统剪贴板上。
// 写入的样例内容带时间戳，格式为 "ManualAid Test Clipboard at {yyyy-mm-dd hh:mm:ss}"，
// 便于在剪贴板上识别测试数据。
fn test_sample() -> String {
    format!(
        "ManualAid Test Clipboard at {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

// 占用剪贴板串行锁，避免与同样会写入剪贴板的 self_check 测试并行运行时互相覆盖
fn lock_clipboard() -> MutexGuard<'static, ()> {
    crate::CLIPBOARD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn test_write_then_read_roundtrip() {
    let _lock = lock_clipboard();
    let sample = test_sample();
    write_clipboard(&sample).expect("write should succeed");
    let text = read_clipboard().expect("read should succeed");
    assert_eq!(text, sample);
}

#[test]
fn test_inspect_clipboard_returns_text() {
    let _lock = lock_clipboard();
    let sample = test_sample();
    write_clipboard(&sample).expect("write should succeed");
    let content = inspect_clipboard().expect("inspect should succeed");
    assert_eq!(content, ClipboardContent::Text(sample));
}

#[test]
fn test_read_clipboard_returns_ok() {
    let _lock = lock_clipboard();
    let result = read_clipboard();
    assert!(result.is_ok(), "reading clipboard should not fail");
}

#[test]
fn mock_read_returns_empty_initially() {
    let mock = MockClipboard::new();
    assert_eq!(mock.read().unwrap(), "");
}

#[test]
fn mock_write_read_roundtrip() {
    let mock = MockClipboard::new();
    mock.write("hello").unwrap();
    assert_eq!(mock.read().unwrap(), "hello");
}

#[test]
fn mock_write_overwrites_previous() {
    let mock = MockClipboard::new();
    mock.write("first").unwrap();
    mock.write("second").unwrap();
    assert_eq!(mock.read().unwrap(), "second");
}

#[test]
fn mock_write_empty_clears() {
    let mock = MockClipboard::new();
    mock.write("content").unwrap();
    mock.write("").unwrap();
    assert_eq!(mock.read().unwrap(), "");
}

#[test]
fn mock_read_error_is_consumed_on_first_call() {
    let mock = MockClipboard::new();
    mock.write("data").unwrap();
    mock.set_read_error("read failed");
    assert_eq!(mock.read(), Err("read failed".to_string()));
    assert_eq!(mock.read().unwrap(), "data");
}

#[test]
fn mock_write_error_is_consumed_on_first_call() {
    let mock = MockClipboard::new();
    mock.set_write_error("write failed");
    assert_eq!(mock.write("data"), Err("write failed".to_string()));
    mock.write("data").unwrap();
    assert_eq!(mock.read().unwrap(), "data");
}
