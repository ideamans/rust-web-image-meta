#!/bin/bash

# Pre-commit hook script for web-image-meta
# Copy this to .git/hooks/pre-commit and make it executable

set -e

echo "Running pre-commit checks..."

# Format check
echo "Checking formatting..."
cargo fmt --check || {
    echo "❌ Code is not formatted. Run 'cargo fmt' to fix."
    exit 1
}

# Clippy check
echo "Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings || {
    echo "❌ Clippy found issues. Fix them before committing."
    exit 1
}

# Test check
echo "Running tests..."
cargo test --quiet || {
    echo "❌ Tests failed. Fix them before committing."
    exit 1
}

# Documentation check
echo "Checking documentation..."
cargo doc --no-deps --quiet || {
    echo "❌ Documentation build failed."
    exit 1
}

echo "✅ All pre-commit checks passed!"