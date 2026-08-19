use super::*;

#[test]
fn time_returns_output_and_elapsed() {
    let (sum, elapsed) = time(|| 1 + 2);
    assert_eq!(sum, 3);
    assert!(elapsed >= Duration::ZERO);
}

#[test]
fn time_measures_sleep_duration() {
    let sleep = Duration::from_millis(20);
    let ((), elapsed) = time(|| std::thread::sleep(sleep));
    assert!(elapsed >= sleep);
}

#[tokio::test]
async fn time_async_returns_output_and_elapsed() {
    let (value, elapsed) = time_async(async { 40 + 2 }).await;
    assert_eq!(value, 42);
    assert!(elapsed >= Duration::ZERO);
}

#[tokio::test]
async fn time_async_measures_sleep_duration() {
    let sleep = Duration::from_millis(20);
    let ((), elapsed) = time_async(tokio::time::sleep(sleep)).await;
    assert!(elapsed >= sleep);
}
