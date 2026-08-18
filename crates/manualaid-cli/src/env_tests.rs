//! Unit tests for the crate-private process environment helpers in `env.rs`.
//! `env.rs` 中 crate 私有进程环境辅助函数的单元测试。

use super::{current_dir, default_message};
use crate::test_support::LOCALE_LOCK;

#[test]
fn default_message_is_localized() {
    let _guard = LOCALE_LOCK.lock().unwrap();
    let version = env!("CARGO_PKG_VERSION");
    i18n::set_locale("en");
    assert_eq!(
        default_message(),
        format!("ManualAid v{version} is running...")
    );
    i18n::set_locale("zh-CN");
    assert_eq!(
        default_message(),
        format!("ManualAid v{version} 正在运行...")
    );
}

#[test]
fn current_dir_resolves() {
    assert!(current_dir().is_ok());
}
