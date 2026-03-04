# tempo-reconcile — Rust workspace

This workspace contains two crates:

| Crate | Path | Published |
|-------|------|-----------|
| `tempo-reconcile` | [`tempo-reconcile/`](tempo-reconcile/) | [crates.io](https://crates.io/crates/tempo-reconcile) |
| `tempo-reconcile-cli` | [`cli/`](cli/) | binary `tempo-reconcile` |

## Build

```bash
# library (all features)
cargo build --all-features

# CLI binary
cargo build -p tempo-reconcile-cli --release
```

## Test

```bash
# all tests, all features
cargo test --all-features

# library only
cargo test -p tempo-reconcile --all-features

# CLI only
cargo test -p tempo-reconcile-cli

# live testnet (requires SENDER_KEY env var)
cargo test --test live_testnet --features watcher -- --nocapture
```

## Lint / format

```bash
cargo clippy --all-features -- -D warnings
cargo fmt --check
cargo doc --no-deps --all-features
```

## Feature flags

See [`tempo-reconcile/README.md`](tempo-reconcile/README.md) for the full feature table.

Core (`memo` + `reconciler`) has **zero** heavyweight dependencies — no async runtime, no network, no TLS.

```
rand          random_salt()
serde         Serialize/Deserialize on all types
export        export_csv / export_json / export_jsonl
watcher       watch_tip20_transfers (tokio + reqwest)
watcher-ws    watch_tip20_transfers_ws (tokio-tungstenite)
webhook       send_webhook + sign — HMAC-SHA256, batching, retries
explorer      ExplorerClient — metadata, balances, history
full          all of the above
```

## Crate structure

```
tempo-reconcile/
  src/
    memo/          encode_memo_v1, decode_memo_v1, issuer_tag_from_namespace, ulid_to_bytes16
    reconciler/    Reconciler, ReconcileStore, InMemoryStore
    export/        export_csv, export_json, export_jsonl, send_webhook, sign
    watcher/       watch_tip20_transfers, get_tip20_transfer_history
    explorer/      ExplorerClient
  tests/           integration tests (mirrors src/ layout)

cli/
  src/
    cmd/memo.rs    memo encode / decode / generate / issuer-tag
    cmd/run.rs     run (reconcile JSONL events against CSV expected)
    cmd/watch.rs   watch (stream on-chain transfers)
    io/            CSV/JSONL parsers for CLI input files
```
