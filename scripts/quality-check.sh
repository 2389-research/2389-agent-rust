#!/bin/bash
set -e

echo "🔍 Running Rust quality checks..."

# 1. Format check (like black --check)
echo "📝 Checking code formatting..."
cargo fmt --check || (echo "❌ Code not formatted. Run: cargo fmt" && exit 1)

# 2. Lint check (like ruff)
echo "🔧 Running clippy lints..."
cargo clippy --all-targets --all-features -- -D warnings || (echo "❌ Clippy warnings found" && exit 1)

# 3. Type check (faster than full compile)
echo "🏗️  Type checking..."
cargo check --all-targets --all-features || (echo "❌ Type errors found" && exit 1)

# 4. Test compilation (don't run yet)
echo "🧪 Checking test compilation..."
cargo test --no-run || (echo "❌ Tests don't compile" && exit 1)

# 5. Documentation check
echo "📚 Checking documentation..."
cargo doc --no-deps --document-private-items || (echo "❌ Documentation errors" && exit 1)

echo "✅ All quality checks passed!"