<#
.SYNOPSIS
    CI 检查脚本（格式、Clippy、文档、测试覆盖率）
.DESCRIPTION
    顺序执行 cargo fmt、cargo clippy、cargo doc，随后运行文档测试，最后通过 cargo llvm-cov 生成覆盖率报告（同时运行所有其余测试）。
    任何步骤失败即退出。
#>

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# 检查 cargo-llvm-cov 是否已安装
Write-Host "========================================"
Write-Host "Checking for cargo-llvm-cov..."
if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
    Write-Host "[ERROR] cargo-llvm-cov not found. Please install: cargo install cargo-llvm-cov"
    exit 1
}

# 1. 格式检查
Write-Host "========================================"
Write-Host ">> Checking formatting..."
cargo fmt -- --check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 2. Clippy（警告视为错误）
Write-Host "========================================"
Write-Host ">> Running Clippy (warnings as errors)..."
cargo clippy -- -D warnings
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 3. 构建文档
Write-Host "========================================"
Write-Host ">> Building documentation..."
cargo doc --no-deps
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 文档测试独立运行，轻量且便于定位文档示例中的问题
Write-Host "========================================"
Write-Host ">> Running doc tests (lightweight)..."
cargo test --doc -q
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# 其他测试由 llvm-cov 在覆盖率生成时一并运行，避免重复执行
# 4. 生成覆盖率报告（同时运行测试）
Write-Host "========================================"
Write-Host ">> Generating coverage report..."
cargo llvm-cov -q --show-missing-lines > coverage_with_lines.txt
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Write-Host "========================================"
Write-Host "[SUCCESS] All CI checks passed."
exit 0
