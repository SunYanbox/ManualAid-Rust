//! The built-in translation wrapper library based on the rust_i18n crate for ManualAid.
//! 用于 ManualAid 的基于 rust_i18n crate 的翻译封装内置库。
//! 
//! # Description
//! `rust_i18n::i18n!()` must be called exactly once in the crate graph —
//! here in the library crate — because `t!` expands to
//! `crate::_rust_i18n_t!(...)` which requires `i18n!()` to have run in the
//! same crate.  Calling it in both lib and bin causes macro-resolution
//! conflicts.  The binary crate uses `i18n::t_str()` instead.
//! # 描述
//! `rust_i18n::i18n!()` 必须在 crate 图中且仅调用一次 —
//! 这里放在库 crate 中 — 因为 `t!` 会展开为
//! `crate::_rust_i18n_t!(...)`，这要求 `i18n!()` 必须在同一个 crate 中
//! 执行过。如果在 lib 和 bin 中同时调用会导致宏解析冲突。
//! 二进制 crate 改为使用 `i18n::t_str()`。
mod init;
use crate::init::_rust_i18n_try_translate;

pub use rust_i18n::set_locale;
pub use rust_i18n::t;

/// Translate a key via the library's i18n backend.
/// 通过库的 i18n 后端翻译一个键值。
/// 
/// # Description
/// This function exists primarily for the binary crate (`main.rs`), which is a
/// separate crate and cannot directly use the `t!` macro: `t!` expands to
/// `crate::_rust_i18n_t!(…)`, which requires the calling crate to have invoked
/// `rust_i18n::i18n!()` — calling it in both lib and bin causes macro-resolution
/// conflicts, so only the library crate owns that invocation.
///
/// Inside the library crate `t!` works fine from sub-modules (e.g.
/// `println!("{{}}", t!("key"))`), but this function deliberately avoids using
/// `t!` internally: doing so would re-enter the proc-macro expansion chain
/// (`t!` → `_rust_i18n_t!` → `rust_i18n::_tr!`) which can stall early-phase
/// resolution for other `t!` call-sites in the same compilation unit.
/// Calling `_rust_i18n_try_translate` directly is equivalent and avoids that
/// hazard.
///
/// If no translation is found, the key itself is returned.
/// # 描述
/// 此函数主要为二进制 crate（`main.rs`）而存在，因为二进制 crate 是一个
/// 独立的 crate，无法直接使用 `t!` 宏：`t!` 会展开为
/// `crate::_rust_i18n_t!(…)`，这要求调用 crate 必须已经调用了
/// `rust_i18n::i18n!()` —— 如果在 lib 和 bin 中同时调用会导致宏解析
/// 冲突，因此只有库 crate 拥有该调用的所有权。
///
/// 在库 crate 内部，`t!` 可以在子模块中正常工作（例如
/// `println!("{{}}", t!("key"))`），但此函数故意避免在内部使用
/// `t!`：这样做会重新进入过程宏展开链
///（`t!` → `_rust_i18n_t!` → `rust_i18n::_tr!`），这可能会阻塞同一编译单元中
/// 其他 `t!` 调用点的早期阶段解析。
/// 直接调用 `_rust_i18n_try_translate` 效果相同，且可以避免该风险。
///
/// 如果键没有对应的翻译，则返回该键本身。
pub fn t_str(key: &str) -> String {
    let locale = &rust_i18n::locale();
    _rust_i18n_try_translate(locale, key)
        .unwrap_or(std::borrow::Cow::Borrowed(key))
        .into_owned()
}
