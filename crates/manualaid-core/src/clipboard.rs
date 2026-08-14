//! System clipboard abstraction with real and mock providers.
//! 系统剪贴板抽象，提供真实实现与测试 Mock。
//!
//! # Description
//! A [`ClipboardProvider`] trait decouples clipboard I/O from business
//! logic so that tests can inject an in-memory [`MockClipboard`] instead
//! of touching the real system clipboard.  The free functions
//! [`read_clipboard`], [`write_clipboard`] and [`inspect_clipboard`]
//! remain available as thin wrappers around [`RealClipboard`] for
//! backward compatibility.
//! # 描述
//! [`ClipboardProvider`] trait 将剪贴板 I/O 与业务逻辑解耦，
//! 使测试可以注入内存中的 [`MockClipboard`] 而非触碰真实系统剪贴板。
//! [`read_clipboard`]、[`write_clipboard`] 和 [`inspect_clipboard`]
//! 保留为 [`RealClipboard`] 的薄包装，保持向后兼容。

use std::cell::RefCell;
use std::sync::Mutex;

/// Abstraction for clipboard read/write operations, enabling dependency
/// injection for testability.
/// 剪贴板读写操作的抽象，支持依赖注入以提升可测试性。
pub trait ClipboardProvider {
    /// Read text content from the clipboard; returns an empty string when
    /// the clipboard holds no text.
    /// 从剪贴板读取文本内容；剪贴板无文本时返回空字符串。
    fn read(&self) -> Result<String, String>;
    /// Write text content to the clipboard, replacing any previous content.
    /// 向剪贴板写入文本内容，覆盖原有内容。
    fn write(&self, text: &str) -> Result<(), String>;
}

/// Real clipboard implementation backed by `arboard`.
/// 基于 `arboard` 的真实剪贴板实现。
pub struct RealClipboard;

impl ClipboardProvider for RealClipboard {
    fn read(&self) -> Result<String, String> {
        read_impl()
    }
    fn write(&self, text: &str) -> Result<(), String> {
        write_impl(text)
    }
}

/// In-memory clipboard for testing; stores a single `String` that can be
/// read back after a write.  Optional errors can be injected via
/// [`set_read_error`](MockClipboard::set_read_error) and
/// [`set_write_error`](MockClipboard::set_write_error).
/// 用于测试的内存剪贴板；存储单个 `String`，写入后可读回。
/// 可通过 [`set_read_error`](MockClipboard::set_read_error) 和
/// [`set_write_error`](MockClipboard::set_write_error) 注入错误。
pub struct MockClipboard {
    content: RefCell<String>,
    read_error: RefCell<Option<String>>,
    write_error: RefCell<Option<String>>,
}

impl MockClipboard {
    /// Create an empty mock clipboard.
    /// 创建一个空的 mock 剪贴板。
    pub fn new() -> Self {
        Self {
            content: RefCell::new(String::new()),
            read_error: RefCell::new(None),
            write_error: RefCell::new(None),
        }
    }
    /// Set an error to be returned by the next `read()` call; subsequent
    /// calls behave normally unless the error is set again.
    /// 设置下次 `read()` 返回的错误；后续调用恢复正常，除非重新设置错误。
    pub fn set_read_error(&self, error: impl Into<String>) {
        *self.read_error.borrow_mut() = Some(error.into());
    }
    /// Set an error to be returned by the next `write()` call; subsequent
    /// calls behave normally unless the error is set again.
    /// 设置下次 `write()` 返回的错误；后续调用恢复正常，除非重新设置错误。
    pub fn set_write_error(&self, error: impl Into<String>) {
        *self.write_error.borrow_mut() = Some(error.into());
    }
}

impl Default for MockClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for MockClipboard {
    fn read(&self) -> Result<String, String> {
        if let Some(err) = self.read_error.borrow_mut().take() {
            return Err(err);
        }
        Ok(self.content.borrow().clone())
    }
    fn write(&self, text: &str) -> Result<(), String> {
        if let Some(err) = self.write_error.borrow_mut().take() {
            return Err(err);
        }
        *self.content.borrow_mut() = text.to_string();
        Ok(())
    }
}

/// The single persistent clipboard handle for the process lifetime.
/// 进程存活期间唯一的剪贴板句柄。
///
/// # Description
/// On Linux/X11, `arboard` destroys its window and hands the selection over to
/// the clipboard manager only when the last `Clipboard` handle is dropped;
/// creating a fresh handle per operation can lose contents written just before
/// the drop. Worse, creating a temporary handle while a persistent handle is
/// alive makes the temporary handle's drop push the reference count back to
/// `MIN_OWNERS`, triggering that same teardown and invalidating the persistent
/// handle, so every read and write must share this single handle.
/// # 描述
/// Linux/X11 的 `arboard` 在最后一个 `Clipboard` 句柄 Drop 时才会销毁窗口并
/// 把选择权交给剪贴板管理器；若每次读写都新建句柄，刚写入就 Drop 会让剪贴板
/// 管理器来不及取走内容。更重要的是，在持有一个长期句柄的同时新建临时句柄，
/// 临时句柄 Drop 时会让引用计数回落到 `MIN_OWNERS`，触发同一销毁流程，使长期
/// 句柄失效。因此所有读写必须共用这一个句柄。
static KEEPER: Mutex<Option<arboard::Clipboard>> = Mutex::new(None);

/// Return the persistent clipboard handle, creating it on first use.
/// 返回进程级剪贴板句柄，首次使用时惰性创建。
fn keeper() -> Result<std::sync::MutexGuard<'static, Option<arboard::Clipboard>>, String> {
    let mut guard = KEEPER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if guard.is_none() {
        *guard =
            Some(arboard::Clipboard::new().map_err(|e| format!("Read Clipboard Failed: {e}"))?);
    }
    Ok(guard)
}

/// Internal read implementation shared by [`RealClipboard`] and the
/// backward-compatible free function [`read_clipboard`].
/// [`RealClipboard`] 与向后兼容自由函数 [`read_clipboard`] 共享的内部读取实现。
fn read_impl() -> Result<String, String> {
    let mut guard = keeper()?;
    let clipboard = guard.as_mut().expect("keeper must be initialized");
    match clipboard.get_text() {
        Ok(text) => Ok(text),
        Err(arboard::Error::ContentNotAvailable) => Ok(String::new()),
        Err(e) => Err(format!("Read Clipboard Failed: {e}")),
    }
}

/// Internal write implementation shared by [`RealClipboard`] and the
/// backward-compatible free function [`write_clipboard`].
/// [`RealClipboard`] 与向后兼容自由函数 [`write_clipboard`] 共享的内部写入实现。
fn write_impl(text: &str) -> Result<(), String> {
    let mut guard = keeper()?;
    let clipboard = guard.as_mut().expect("keeper must be initialized");
    clipboard
        .set_text(text)
        .map_err(|e| format!("写入剪贴板失败：{e}"))
}

/// Describes what the system clipboard currently holds.
/// 描述系统剪贴板当前持有的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardContent {
    /// The clipboard holds text.
    /// 剪贴板包含文本。
    Text(String),
    /// The clipboard holds an image with the given pixel dimensions.
    /// 剪贴板包含指定像素尺寸的图像。
    Image { width: usize, height: usize },
    /// The clipboard holds no readable text or image content.
    /// 剪贴板中没有可读取的文本或图像内容。
    Empty,
}

/// Inspect the system clipboard and describe its current content.
/// 检查系统剪贴板并描述其当前内容。
///
/// # Description
/// Unlike [`read_clipboard`], this distinguishes text, image, and empty
/// content so a caller can report what is actually on the clipboard.
/// Errors during the inspection (e.g. permission denied, clipboard busy)
/// are returned as a human-readable message.
/// # Test notes
/// This function is not part of the [`ClipboardProvider`] trait because it
/// has no callers outside this module.  Its image / empty / error branches
/// require non-text data on the real clipboard and are not covered by
/// unit tests.
/// # 描述
/// 与 [`read_clipboard`] 不同，本函数区分文本、图像和空内容，
/// 便于调用方报告剪贴板上实际存在的内容。
/// 检查过程中发生的错误（如权限不足、剪贴板被占用）以人类可读的错误信息返回。
/// # 测试说明
/// 本函数不属于 [`ClipboardProvider`] trait，因为模块外无调用方。
/// 其图像 / 空内容 / 错误分支需要在真实剪贴板上放置非文本数据，
/// 不由单元测试覆盖。
pub fn inspect_clipboard() -> Result<ClipboardContent, String> {
    let mut guard = keeper()?;
    let clipboard = guard.as_mut().expect("keeper must be initialized");
    match clipboard.get_text() {
        Ok(text) => Ok(ClipboardContent::Text(text)),
        Err(arboard::Error::ContentNotAvailable) => match clipboard.get_image() {
            Ok(image) => Ok(ClipboardContent::Image {
                width: image.width,
                height: image.height,
            }),
            Err(arboard::Error::ContentNotAvailable) => Ok(ClipboardContent::Empty),
            Err(e) => Err(format!("Read Clipboard Failed: {e}")),
        },
        Err(e) => Err(format!("Read Clipboard Failed: {e}")),
    }
}

/// Read the current text content from the system clipboard.
/// 从系统剪贴板读取当前文本内容。
///
/// # Description
/// This is a backward-compatible wrapper that delegates to
/// [`RealClipboard::read`].  New code should accept a
/// `&impl ClipboardProvider` parameter instead of calling this function
/// directly, so that tests can inject a [`MockClipboard`].
/// # 描述
/// 向后兼容包装，委托给 [`RealClipboard::read`]。
/// 新代码应接受 `&impl ClipboardProvider` 参数而非直接调用本函数，
/// 以便测试注入 [`MockClipboard`]。
pub fn read_clipboard() -> Result<String, String> {
    RealClipboard.read()
}

/// Write text into the system clipboard, replacing any previous content.
/// 将文本写入系统剪贴板，覆盖原有内容。
///
/// # Description
/// This is a backward-compatible wrapper that delegates to
/// [`RealClipboard::write`].  New code should accept a
/// `&impl ClipboardProvider` parameter instead of calling this function
/// directly, so that tests can inject a [`MockClipboard`].
/// # 描述
/// 向后兼容包装，委托给 [`RealClipboard::write`]。
/// 新代码应接受 `&impl ClipboardProvider` 参数而非直接调用本函数，
/// 以便测试注入 [`MockClipboard`]。
pub fn write_clipboard(text: impl AsRef<str>) -> Result<(), String> {
    RealClipboard.write(text.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
