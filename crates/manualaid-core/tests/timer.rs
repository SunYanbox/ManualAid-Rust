use std::time::Duration;

use manualaid_core::timer::{Timer, time, time_async};

/// `Timer` measures a sleep and reports it in nanoseconds.
/// `Timer` 能计量一段 sleep 并以纳秒报告。
#[test]
fn timer_elapsed_nanos_after_sleep() {
    let timer = Timer::start();
    let sleep = Duration::from_millis(20);
    std::thread::sleep(sleep);
    assert!(timer.elapsed() >= sleep);
    assert!(timer.elapsed_nanos() >= sleep.as_nanos());
}

/// `time` returns the closure output together with the elapsed time.
/// `time` 同时返回闭包输出与耗时时长。
#[test]
fn time_returns_value_and_elapsed() {
    let (value, elapsed) = time(|| "hello".to_string());
    assert_eq!(value, "hello");
    assert!(elapsed >= Duration::ZERO);
}

/// `time_async` returns the future output together with the elapsed time.
/// `time_async` 同时返回 future 输出与耗时时长。
#[tokio::test]
async fn time_async_returns_value_and_elapsed() {
    let (value, elapsed) = time_async(async { 21 * 2 }).await;
    assert_eq!(value, 42);
    assert!(elapsed >= Duration::ZERO);
}

/// `time_async` measures an async sleep.
/// `time_async` 能计量一段异步 sleep。
#[tokio::test]
async fn time_async_measures_async_sleep() {
    let sleep = Duration::from_millis(20);
    let ((), elapsed) = time_async(tokio::time::sleep(sleep)).await;
    assert!(elapsed >= sleep);
}
