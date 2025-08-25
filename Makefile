# Simple dev workflow

.PHONY: build run test fmt fmt-check lint check ci audit audit-fix

build:
	cargo build

run:
	cargo run

test:
	cargo test --all

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	rustup component add clippy >/dev/null 2>&1 || true
	cargo clippy -- -D warnings

check:
	cargo check --all

ci: check fmt-check lint test

audit:
	cargo audit

audit-fix:
	cargo audit fix || true
