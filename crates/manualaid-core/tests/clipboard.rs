use std::sync::{Mutex, MutexGuard};

use manualaid_core::clipboard::{
    ClipboardContent, ClipboardProvider, MockClipboard, inspect_clipboard, read_clipboard,
    write_clipboard,
};

// The clipboard is system-wide shared state. Any test that writes to it must
// hold this lock so parallel tests cannot overwrite each other.
// 剪贴板是系统级共享状态，任何写入剪贴板的测试必须持有此锁，
// 避免并行测试互相覆盖。
pub(crate) static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

/// Saves the original clipboard text and restores it on drop.
/// 保存剪贴板原始文本并在 drop 时恢复。
struct ClipboardRestore {
    original: String,
}

impl ClipboardRestore {
    fn save() -> Self {
        Self {
            original: read_clipboard().unwrap_or_default(),
        }
    }
}

impl Drop for ClipboardRestore {
    fn drop(&mut self) {
        let _ = write_clipboard(&self.original);
    }
}

fn test_sample() -> String {
    format!(
        "ManualAid Test Clipboard at {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

fn lock_clipboard() -> MutexGuard<'static, ()> {
    crate::CLIPBOARD_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
#[ignore = "manual: requires the real system clipboard (AGENTS.md policy)"]
fn test_write_then_read_roundtrip() {
    let _lock = lock_clipboard();
    let _restore = ClipboardRestore::save();
    let sample = test_sample();
    write_clipboard(&sample).expect("write should succeed");
    let text = read_clipboard().expect("read should succeed");
    assert_eq!(text, sample);
}

#[test]
#[ignore = "manual: requires the real system clipboard (AGENTS.md policy)"]
fn test_inspect_clipboard_returns_text() {
    let _lock = lock_clipboard();
    let _restore = ClipboardRestore::save();
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
