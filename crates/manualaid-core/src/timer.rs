//! Measure the execution time of functions and code blocks using
//! [`std::time::Instant`].
//! 使用 [`std::time::Instant`] 计量函数和代码块的执行时间。
//!
//! # Description
//! This module is a standalone, reusable primitive: it is intentionally not
//! wired into any other module in this crate. Application layers call
//! [`Timer`], [`time`] or [`time_async`] at runtime wherever they need to
//! record how long an operation took, and decide themselves whether to log,
//! display or persist the measurement.
//!
//! Durations come from the monotonic `Instant` clock, so they measure wall
//! time on the current machine; precision differences between devices are
//! acceptable. The nanosecond unit is `u128`, produced by
//! [`Duration::as_nanos`].
//! # 描述
//! 本模块是一个独立、可复用的原语，有意不内嵌到本 crate 的任何其他模块。
//! 应用层在运行时需要记录某项操作耗时的地方调用 [`Timer`]、[`time`] 或
//! [`time_async`]，并由调用方自行决定如何记录、展示或持久化这一测量结果。
//!
//! 时长来自单调的 `Instant` 时钟，测量的是当前机器上的墙钟时间；设备之间的
//! 精度差异可以接受。纳秒单位为 `u128`，由 [`Duration::as_nanos`] 产生。
use std::time::{Duration, Instant};

/// A stopwatch measuring elapsed time with `std::time::Instant`.
/// 基于 `std::time::Instant` 的秒表式计时器。
///
/// # Description
/// Create one with [`Timer::start`], read the elapsed time at any point with
/// [`Timer::elapsed`] or [`Timer::elapsed_nanos`], and restart the
/// measurement with [`Timer::reset`]. Useful for timing operations whose
/// start and end are not adjacent, including `async` code.
/// # 描述
/// 用 [`Timer::start`] 创建，随时通过 [`Timer::elapsed`] 或
/// [`Timer::elapsed_nanos`] 读取已耗时长，并用 [`Timer::reset`] 重新开始
/// 计时。适合起点与终点不相邻的操作，包括 `async` 代码。
#[derive(Debug)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer at the current instant.
    /// 从当前时刻开始一个新的计时器。
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Return the time elapsed since the timer was started (or last reset).
    /// 返回自计时器创建（或上次 [`Timer::reset`]）以来的已耗时长。
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Return the elapsed time in nanoseconds, equivalent to
    /// `elapsed().as_nanos()`.
    /// 以纳秒为单位返回已耗时长，等价于 `elapsed().as_nanos()`。
    pub fn elapsed_nanos(&self) -> u128 {
        self.elapsed().as_nanos()
    }

    /// Reset the timer so the measurement restarts from the current instant.
    /// 重置计时器，从当前时刻开始重新计算耗时时长。
    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}

/// Run a synchronous closure and return both its output and the elapsed
/// time. The elapsed time is available as nanoseconds through
/// `Duration::as_nanos()`.
/// 执行一个同步闭包并同时返回其输出与耗时时长，耗时时长可通过
/// `Duration::as_nanos()` 以纳秒为单位获取。
pub fn time<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let timer = Timer::start();
    let output = f();
    (output, timer.elapsed())
}

/// Await a future and return both its output and the elapsed time. The
/// elapsed time is available as nanoseconds through `Duration::as_nanos()`.
/// 等待一个 future 并同时返回其输出与耗时时长，耗时时长可通过
/// `Duration::as_nanos()` 以纳秒为单位获取。
pub async fn time_async<T, F>(f: F) -> (T, Duration)
where
    F: std::future::Future<Output = T>,
{
    let timer = Timer::start();
    let output = f.await;
    (output, timer.elapsed())
}

#[cfg(test)]
mod tests;
