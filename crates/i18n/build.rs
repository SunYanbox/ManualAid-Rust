//! Rebuild the `i18n` crate when locale files change, because the
//! `rust_i18n` proc-macro reads them at expansion time without declaring a
//! dependency that Cargo would otherwise track.
//! 当 locale 文件变化时重建 `i18n` crate：`rust_i18n` 过程宏在展开时读取
//! 这些文件，但不会声明 Cargo 可跟踪的依赖。

use std::path::Path;

fn main() {
    let locales = Path::new("locales");
    println!("cargo:rerun-if-changed={}", locales.display());
    if let Ok(entries) = std::fs::read_dir(locales) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
}
