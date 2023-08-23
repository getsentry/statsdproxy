test:
	cargo test --all-features
.PHONY: test

lint:
	cargo clippy -- -D warnings
.PHONY: lint
