@echo off
setlocal enabledelayedexpansion

echo ========================================
echo Checking for cargo-llvm-cov...
where cargo-llvm-cov >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo-llvm-cov not found. Please install: cargo install cargo-llvm-cov
    exit /b 1
)

echo ========================================
echo ^>^> Checking formatting...
cargo fmt -- --check
if errorlevel 1 exit /b 1

echo ========================================
echo ^>^> Running Clippy (warnings as errors)...
cargo clippy -- -D warnings
if errorlevel 1 exit /b 1

echo ========================================
echo ^>^> Building documentation...
cargo doc --no-deps
if errorlevel 1 exit /b 1

:: 文档测试独立运行，轻量且便于定位文档示例中的问题
echo ========================================
echo ^>^> Running doc tests (lightweight)...
cargo test --doc -q
if errorlevel 1 exit /b 1

:: 其他测试由 llvm-cov 在覆盖率生成时一并运行，避免重复执行
echo ========================================
echo ^>^> Generating coverage report...
cargo llvm-cov -q --show-missing-lines > coverage_with_lines.txt
if errorlevel 1 exit /b 1

echo ========================================
echo [SUCCESS] All CI checks passed.
exit /b 0
