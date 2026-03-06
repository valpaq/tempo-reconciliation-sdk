.PHONY: build test lint fmt check clean coverage doc example audit test-nonces test-cli publish-dry e2e

build:
	cargo build --workspace --release

test:
	cargo test --workspace --all-features

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

check:
	cargo fmt --check && cargo clippy --workspace --all-targets --all-features -- -D warnings

clean:
	cargo clean

coverage:
	cargo llvm-cov --workspace --all-features --html

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
	cargo doc --no-deps --open

example:
	cargo run --example showcase

audit:
	cargo audit

test-nonces:
	cargo test -p tempo-reconcile-nonces

test-cli:
	cargo test -p tempo-reconcile-cli

publish-dry:
	cargo publish -p tempo-reconcile --dry-run

e2e:
	TEMPO_LIVE=1 cargo test --test live_testnet --features watcher -- --nocapture
