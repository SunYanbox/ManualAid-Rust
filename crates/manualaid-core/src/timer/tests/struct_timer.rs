use super::*;

#[test]
fn timer_elapsed_is_monotonic() {
    let timer = Timer::start();
    let first = timer.elapsed();
    let second = timer.elapsed();
    assert!(second >= first);
}

#[test]
fn timer_elapsed_nanos_matches_duration_within_tolerance() {
    let timer = Timer::start();
    std::thread::sleep(Duration::from_millis(20));
    // The two readings are taken microseconds apart, so they must agree
    // within a small tolerance rather than exactly.
    let duration_nanos = timer.elapsed().as_nanos();
    let nanos = timer.elapsed_nanos();
    assert!(nanos.abs_diff(duration_nanos) < Duration::from_millis(1).as_nanos());
}

#[test]
fn timer_elapsed_after_sleep_is_at_least_sleep() {
    let timer = Timer::start();
    let sleep = Duration::from_millis(20);
    std::thread::sleep(sleep);
    assert!(timer.elapsed() >= sleep);
}

#[test]
fn timer_reset_restarts_measurement() {
    let mut timer = Timer::start();
    std::thread::sleep(Duration::from_millis(20));
    assert!(timer.elapsed() >= Duration::from_millis(20));

    timer.reset();
    std::thread::sleep(Duration::from_millis(20));
    assert!(timer.elapsed() >= Duration::from_millis(20));
}
