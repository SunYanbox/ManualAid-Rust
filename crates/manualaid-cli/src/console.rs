//! Process-wide console output sink: tests redirect every console write into
//! an in-process string instead of the real terminal, so `cargo test` never
//! clears or pollutes the user's terminal regardless of how it is launched.
//! 进程级控制台输出出口：测试把所有控制台输出重定向到进程内字符串变量，
//! 使 `cargo test` 无论以何种方式启动都不会清屏或污染用户终端。

use std::io::{self, Write};
use std::sync::Mutex;

/// The active capture buffer; `None` routes writes to the real stdout.
/// 当前捕获缓冲区；`None` 时写入真实 stdout。
static SINK: Mutex<Option<Vec<u8>>> = Mutex::new(None);

/// Serializes `capture()` calls so concurrent tests never share a buffer.
/// 串行化 `capture()` 调用，避免并发测试共享同一个缓冲区。
static CAPTURE_LOCK: Mutex<()> = Mutex::new(());

/// Write `text` into the active capture buffer, or to the real stdout when
/// no capture is active; write errors are ignored so a broken pipe never
/// panics the process.
/// 把 `text` 写入当前捕获缓冲区；未捕获时写入真实 stdout。写入错误被
/// 忽略，管道断开不会导致进程 panic。
pub(crate) fn write_text(text: &str) {
    let mut sink = SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    match sink.as_mut() {
        Some(buf) => buf.extend_from_slice(text.as_bytes()),
        None => {
            let _ = io::stdout().lock().write_all(text.as_bytes());
        }
    }
}

/// Flush pending console output; nothing to flush while capturing, because
/// the buffer is written synchronously.
/// 冲刷待输出的控制台内容；捕获期间无需冲刷，缓冲区为同步写入。
pub(crate) fn flush() {
    let sink = SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if sink.is_some() {
        return;
    }
    let _ = io::stdout().flush();
}

/// Whether a test capture is currently active.
/// 当前是否处于测试捕获状态。
pub(crate) fn is_capturing() -> bool {
    SINK.lock()
        .map(|sink| sink.is_some())
        .unwrap_or_else(|poisoned| poisoned.into_inner().is_some())
}

/// A `Write` adapter routing bytes into the capture buffer or the real
/// stdout, so pager-style helpers honor an active capture.
/// 把字节写入捕获缓冲区或真实 stdout 的 `Write` 适配器，使分页器等
/// 辅助函数尊重当前捕获状态。
pub(crate) struct ConsoleWriter;

impl Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut sink = SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match sink.as_mut() {
            Some(capture) => capture.extend_from_slice(buf),
            None => io::stdout().lock().write_all(buf)?,
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        flush();
        Ok(())
    }
}

/// Format `args` into the console sink without a trailing newline.
/// 把 `args` 格式化到控制台出口，不追加换行。
pub(crate) fn text(args: std::fmt::Arguments<'_>) {
    write_text(&args.to_string());
}

/// Format `args` into the console sink with a trailing newline.
/// 把 `args` 格式化到控制台出口，并追加换行。
pub(crate) fn line(args: std::fmt::Arguments<'_>) {
    write_text(&format!("{args}\n"));
}

/// Print without a trailing newline; routed through the capture buffer.
/// 输出不追加换行；随捕获状态重定向。
macro_rules! out_print {
    ($($arg:tt)*) => {
        $crate::console::text(format_args!($($arg)*))
    };
}

/// Print with a trailing newline; routed through the capture buffer.
/// 输出并追加换行；随捕获状态重定向。
macro_rules! out_println {
    () => {
        $crate::console::line(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::console::line(format_args!($($arg)*))
    };
}

pub(crate) use out_print;
pub(crate) use out_println;

/// Start redirecting console output into an in-process buffer; the buffer is
/// reset on drop. Public because integration tests link the non-test library
/// and must redirect command output the same way unit tests do; held across
/// an await in `#[tokio::test]` (current-thread runtime), matching the
/// existing test-lock pattern.
/// 开始把控制台输出重定向到进程内缓冲区；Drop 时缓冲区清空恢复。对集成
/// 测试公开，因为它们链接非 test 库，需要与单元测试一样重定向命令输出；
/// 在 `#[tokio::test]`（current-thread runtime）中可跨 await 持有，与现有
/// 测试锁的模式一致。
pub fn capture() -> Capture {
    let lock = CAPTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Vec::new());
    Capture { _lock: lock }
}

/// Guard owning the capture buffer for the duration of one test.
/// 持有整个测试期间的捕获缓冲区的守卫。
pub struct Capture {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for Capture {
    fn drop(&mut self) {
        *SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

impl Capture {
    /// The text captured so far.
    /// 到目前为止捕获到的文本。
    pub fn text(&self) -> String {
        let sink = SINK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8(
            sink.as_ref()
                .expect("capture buffer must be active")
                .clone(),
        )
        .expect("captured output must be UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_redirects_writes_and_text_returns_them() {
        let capture = capture();
        write_text("hello");
        crate::console::line(format_args!(" {}", "world"));
        assert_eq!(capture.text(), "hello world\n");
    }

    #[test]
    fn is_capturing_reflects_the_active_capture() {
        let capture = capture();
        assert!(is_capturing());
        drop(capture);
        assert!(!is_capturing());
    }

    #[test]
    fn capture_resets_the_buffer_on_drop() {
        let first_capture = capture();
        write_text("first");
        drop(first_capture);
        let second_capture = capture();
        assert_eq!(second_capture.text(), "");
        write_text("second");
        assert_eq!(second_capture.text(), "second");
    }

    #[test]
    fn console_writer_routes_into_the_capture() {
        let capture = capture();
        let mut writer = ConsoleWriter;
        writer.write_all(b"a\nb").unwrap();
        writer.flush().unwrap();
        assert_eq!(capture.text(), "a\nb");
    }

    #[test]
    fn macros_format_into_the_capture_buffer() {
        let capture = capture();
        crate::console::out_print!("{}{}", "x", "y");
        crate::console::out_println!("{count}", count = 3);
        assert_eq!(capture.text(), "xy3\n");
    }
}
