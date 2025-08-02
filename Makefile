.PHONY: test lint fmt

# テストの実行（キャッシュクリア付き）
test:
	@echo "Clearing test cache..."
	@rm -rf target/debug/deps/*test*
	@rm -rf target/debug/.fingerprint/*test*
	@echo "Running tests..."
	@cargo test

# Lintチェック
lint:
	@echo "Running clippy..."
	@cargo clippy -- -D warnings

# フォーマット
fmt:
	@echo "Running rustfmt..."
	@cargo fmt