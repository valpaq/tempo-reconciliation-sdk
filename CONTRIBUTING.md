# Contributing

## Prerequisites

- Rust 1.70+ (`rustup update stable`)
- Node.js 18+ and pnpm 10 (`npm i -g pnpm@10`)

## Running tests

```bash
# Rust
cd rs
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
cargo doc --no-deps --all-features

# TypeScript
cd ts
pnpm install
pnpm test
pnpm build
```

## Adding test vectors

Test vectors live in `spec/vectors.json`. To add one:

1. Add the entry to the `positive` or `negative` array in `spec/vectors.json`.
2. Run `cd rs && cargo test --all-features` — `tests/vectors.rs` reads the file directly.
3. Run `cd ts && pnpm test` — the TS vector tests do the same.

Both suites must pass before opening a PR.

## Pull request checklist

- CI passes (test, clippy, fmt, doc)
- New public functions have doc comments (`///`)
- New behavior has a test
- `spec/vectors.json` updated if memo encoding changed
- No `unwrap()` or `unsafe` in production code paths

## Commit style

Conventional commits: `feat:`, `fix:`, `test:`, `docs:`, `chore:`, `refactor:`.

Examples:
```
feat: add `from` filter to WatchConfig
fix: reject reserved memo type codes 0x06-0x0E
test: add holistic report test covering all 8 MatchStatus variants
docs: sync MEMO-SPEC vector counts with spec/vectors.json
```
