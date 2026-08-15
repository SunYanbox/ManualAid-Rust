#!/usr/bin/env bash
set -euo pipefail  # 出错即停、管道失败即停、未定义变量报错

# 检查 cargo-llvm-cov 是否已安装
echo "========================================"
echo "Checking for cargo-llvm-cov..."
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "[ERROR] cargo-llvm-cov not found. Please install: cargo install cargo-llvm-cov"
    exit 1
fi

# 1. 格式检查
echo "========================================"
echo ">> Checking formatting..."
cargo fmt -- --check

# 2. Clippy（警告视为错误）
echo "========================================"
echo ">> Running Clippy (warnings as errors)..."
cargo clippy -- -D warnings

# 3. 构建文档
echo "========================================"
echo ">> Building documentation..."
cargo doc --no-deps

# 文档测试独立运行，轻量且便于定位文档示例中的问题
echo "========================================"
echo ">> Running doc tests (lightweight)..."
cargo test --doc -q

# 其他测试由 llvm-cov 在覆盖率生成时一并运行，避免重复执行
# 4. 生成覆盖率报告（同时运行测试）
echo "========================================"
echo ">> Generating coverage report..."
cargo llvm-cov -q --show-missing-lines > coverage_with_lines.txt

echo "========================================"
echo "[SUCCESS] All CI checks passed."
