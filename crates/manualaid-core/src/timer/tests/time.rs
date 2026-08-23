use super::*;

#[test]
fn time_returns_output_and_elapsed() {
    let (sum, elapsed) = time(|| 1 + 2);
    assert_eq!(sum, 3);
    // No sleep is involved, so the elapsed time must stay well under a second.
    // 无等待操作，耗时应远小于 1 秒。
    assert!(elapsed < Duration::from_secs(1));
}

#[test]
fn time_measures_sleep_duration() {
    let ((), elapsed) = time(|| std::thread::sleep(SLEEP));
    assert!(elapsed >= SLEEP);
}

#[tokio::test]
async fn time_async_returns_output_and_elapsed() {
    let (value, elapsed) = time_async(async { 40 + 2 }).await;
    assert_eq!(value, 42);
    assert!(elapsed < Duration::from_secs(1));
}

#[tokio::test]
async fn time_async_measures_sleep_duration() {
    let ((), elapsed) = time_async(tokio::time::sleep(SLEEP)).await;
    assert!(elapsed >= SLEEP);
}
