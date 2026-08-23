use super::*;

mod struct_timer;
mod time;

/// Short sleep used by timer tests to keep runs fast while still observable.
/// 计时器测试所用的短睡眠，既保持测试快速又足以被观测到。
const SLEEP: Duration = Duration::from_millis(10);
