test:
	cargo test
.PHONY: test

lint:
	cargo clippy -- -D warnings
.PHONY: lint