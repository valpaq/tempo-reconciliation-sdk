.PHONY: build test lint fmt check clean coverage doc example audit test-cli publish-dry e2e

build:
	cargo build --release

test:
	cargo test --all-features

lint:
	cargo clippy --all-targets --all-features -- -D warnings

fmt:
	cargo fmt

check:
	cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings

clean:
	cargo clean

coverage:
	cargo llvm-cov --all-features --html

doc:
	cargo doc --no-deps --open
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps

example:
	cargo run --example showcase

audit:
	cargo audit

test-cli:
	cargo test -p tempo-reconcile-cli

publish-dry:
	cargo publish -p tempo-reconcile --dry-run

e2e:
	TEMPO_LIVE=1 cargo test --test live_testnet --features watcher -- --nocapture
