//! System clipboard read / write via `arboard`.
//! 通过 `arboard` 实现系统剪贴板的读取/写入。
//!
//! 说明：当前阶段暂时放弃对剪贴板相关测试进行 Mock 与还原。
//! 测试（包括 `self_check` 的剪贴板检查）写入的样例内容会直接保留在系统剪贴板上，
//! 内容格式为 `ManualAid Test Clipboard at {yyyy-mm-dd hh:mm:ss}`，便于识别测试数据。

use std::sync::Mutex;

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

/// Read the current text content from the system clipboard.
/// 从系统剪贴板读取当前文本内容。
///
/// # Description
/// Returns an empty string `""` when the clipboard contains no text
/// (it is empty, or holds non-text data such as an image). Errors during
/// the read (e.g. permission denied, clipboard busy) are returned as a
/// human-readable message.
/// # Test notes
/// The system clipboard cannot be mocked or restored in tests, and
/// reaching the no-text / error branches below would require leaving
/// non-text data (such as images) on the clipboard. To avoid cluttering
/// the user's clipboard, these branches are not required to have high
/// test coverage.
/// # 描述
/// 当剪贴板中没有文本内容（为空，或包含图片等非文本数据）时，返回空字符串 `""`。
/// 读取过程中发生的错误（如权限不足、剪贴板被占用）以人类可读的错误信息返回。
/// # 测试说明
/// 系统剪贴板无法在测试中被 Mock 或还原，而覆盖无文本 / 错误分支需要在剪贴板上
/// 留下图片等非文本数据。为避免在用户剪贴板上产生多余的"垃圾"内容，
/// 这些分支不要求高测试覆盖率。
pub fn read_clipboard() -> Result<String, String> {
    let mut guard = keeper()?;
    let clipboard = guard.as_mut().expect("keeper must be initialized");
    match clipboard.get_text() {
        Ok(text) => Ok(text),
        Err(arboard::Error::ContentNotAvailable) => Ok(String::new()),
        Err(e) => Err(format!("Read Clipboard Failed: {e}")),
    }
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
/// The system clipboard cannot be mocked or restored in tests, and
/// reaching the image / empty / error branches below would require
/// leaving non-text data (such as images) on the clipboard. To avoid
/// cluttering the user's clipboard, these branches are not required to
/// have high test coverage.
/// # 描述
/// 与 [`read_clipboard`] 不同，本函数区分文本、图像和空内容，
/// 便于调用方报告剪贴板上实际存在的内容。
/// 检查过程中发生的错误（如权限不足、剪贴板被占用）以人类可读的错误信息返回。
/// # 测试说明
/// 系统剪贴板无法在测试中被 Mock 或还原，而覆盖图像 / 空内容 / 错误分支
/// 需要在剪贴板上留下图片等非文本数据。为避免在用户剪贴板上产生多余的
/// "垃圾"内容，这些分支不要求高测试覆盖率。
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

/// Write text into the system clipboard, replacing any previous content.
/// 将文本写入系统剪贴板，覆盖原有内容。
///
/// # Description
/// An empty string is also accepted (clears the clipboard). Failures during
/// the write are returned as a human-readable message.
/// # Test notes
/// The system clipboard cannot be mocked or restored in tests, so tests
/// calling this function intentionally leave the written text on the
/// clipboard. To avoid extra clipboard writes, this function is not
/// required to have high test coverage.
/// # 描述
/// 空字符串也可以写入（用于清空剪贴板）。写入失败时以人类可读的错误信息返回。
/// 写入成功后句柄保留在进程内，Linux 上不会因句柄过早 Drop 而丢失内容。
/// # 测试说明
/// 系统剪贴板无法在测试中被 Mock 或还原，调用本函数的测试会有意把文本留在剪贴板上。
/// 为避免产生多余的剪贴板写入，本函数不要求高测试覆盖率。
pub fn write_clipboard(text: impl AsRef<str>) -> Result<(), String> {
    let mut guard = keeper()?;
    let clipboard = guard.as_mut().expect("keeper must be initialized");
    clipboard
        .set_text(text.as_ref())
        .map_err(|e| format!("写入剪贴板失败：{e}"))
}
