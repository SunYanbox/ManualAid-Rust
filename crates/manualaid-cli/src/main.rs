use std::process::ExitCode;

use clap::Parser;
use manualaid_cli::cli::Cli;

// `ExitCode` lets main return normally so the coverage runtime's atexit
// flush runs; `std::process::exit` would skip it and produce empty
// profraw files under `cargo llvm-cov`.
// 用 `ExitCode` 让 main 正常返回，覆盖率运行时的 atexit 冲刷才能执行；
// 改用 `std::process::exit` 会跳过冲刷，生成空的 profraw 文件。
fn main() -> ExitCode {
    ExitCode::from(manualaid_cli::commands::run_main(Cli::parse()) as u8)
}
