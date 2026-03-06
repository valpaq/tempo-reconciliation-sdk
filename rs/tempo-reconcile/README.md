# tempo-reconcile

[![Crates.io](https://img.shields.io/crates/v/tempo-reconcile)](https://crates.io/crates/tempo-reconcile)
[![docs.rs](https://docs.rs/tempo-reconcile/badge.svg)](https://docs.rs/tempo-reconcile)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](../../LICENSE)

Reconciliation library for TIP-20 payments on Tempo. Implements the
[TEMPO-RECONCILE-MEMO-001](../../docs-public/MEMO-SPEC.md) bytes32 memo standard.

Does not overlap with the official Tempo SDK — no wallet, no signing.
This is the **receiving side**: decode memos, watch transfers, match payments, export results.

## Install

```toml
[dependencies]
tempo-reconcile = "0.1"
```

## Features

| Feature | Default | Adds | Description |
|---------|---------|------|-------------|
| `rand` | no | rand | `random_salt()` — 7-byte random salt generation |
| `serde` | no | serde | `Serialize` / `Deserialize` on all types |
| `export` | no | serde_json | `export_csv`, `export_json`, `export_jsonl` |
| `watcher` | no | tokio, reqwest | `get_tip20_transfer_history`, `watch_tip20_transfers` |
| `webhook` | no | hmac, sha2 (+ watcher, rand) | `send_webhook` — HMAC-SHA256, batching, retries; `sign` — raw HMAC-SHA256 hex |
| `watcher-ws` | no | tokio-tungstenite (+ watcher) | WebSocket push watcher with auto-reconnect |
| `explorer` | no | reqwest, tokio, serde, serde_json | Tempo Explorer REST client: metadata, balances, history |
| `full` | no | all of the above | Everything |

Default features are intentionally empty — core (memo + reconciler) has zero heavyweight deps.

## Quick start

```rust
use tempo_reconcile::{
    issuer_tag_from_namespace, encode_memo_v1, EncodeMemoV1Params,
    Reconciler, ReconcilerOptions, ExpectedPayment, PaymentEvent, MemoType,
};

// 1. Derive a deterministic issuer tag for your namespace.
let issuer_tag = issuer_tag_from_namespace("my-company");

// 2. Encode a structured memo (bytes32).
let memo = encode_memo_v1(&EncodeMemoV1Params {
    memo_type: MemoType::Invoice,
    issuer_tag,
    ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
    salt: None,           // default: seven zero bytes
}).unwrap();
// memo == "0x01..." (66-char hex, "0x" + 64 hex digits)

// 3. Register the expected payment.
let mut reconciler = Reconciler::new(ReconcilerOptions::new()).unwrap();
reconciler.expect(ExpectedPayment {
    memo_raw: memo.clone(),
    token: "0x20c0000000000000000000000000000000000000".to_string(), // pathUSD
    to: "0xrecipient".to_string(),
    amount: 10_000_000, // 10 USDC (6 decimals)
    from: None,
    due_at: None,
    meta: None,
}).unwrap();

// 4. Ingest an observed on-chain event (from your watcher / event log).
let event = PaymentEvent {
    chain_id: 42431,
    block_number: 1,
    tx_hash: "0xdeadbeef".to_string(),
    log_index: 0,
    token: "0x20c0000000000000000000000000000000000000".to_string(),
    from: "0xsender".to_string(),
    to: "0xrecipient".to_string(),
    amount: 10_000_000,
    memo_raw: Some(memo),
    memo: None,
    timestamp: None,
};

let result = reconciler.ingest(event);
assert_eq!(result.status.as_str(), "matched");

// 5. Generate a report.
let report = reconciler.report();
println!("matched: {}", report.summary.matched_count);
```

## Memo layout (TEMPO-RECONCILE-MEMO-001)

```
byte 0:       type code   0x01-0x0F for v1 types
bytes 1-8:    issuerTag   uint64 big-endian = keccak256(namespace)[0:8]
bytes 9-24:   id16        ULID binary (16 bytes)
bytes 25-31:  salt        7 bytes, default zeros
```

**Type codes:** `invoice` (0x01), `payroll` (0x02), `refund` (0x03),
`batch` (0x04), `subscription` (0x05), `custom` (0x0F).

Memo is a reference. No PII on-chain — customer data lives off-chain.

## Match statuses

| Status | Meaning |
|--------|---------|
| `matched` | Exact (or within tolerance) amount, all constraints pass |
| `partial` | Underpayment accumulated (requires `allow_partial = true`) |
| `unknown_memo` | Memo not in expected list (or issuerTag filtered out) |
| `no_memo` | Transfer without memo field |
| `mismatch_amount` | Wrong amount (outside tolerance) |
| `mismatch_token` | Wrong token contract |
| `mismatch_party` | Wrong sender or recipient |
| `expired` | Payment after `due_at` (requires `reject_expired = true`) |

## Export

```rust
use tempo_reconcile::{export_csv, export_json, export_jsonl};

let csv = export_csv(&report.matched);
let json = export_json(&report.issues);
let jsonl = export_jsonl(&[]);
```

## Minimal footprint

The default feature set is intentionally empty — `memo/` and `reconciler/` have no async runtime,
no network, no TLS, and no allocations in the hot path beyond the hex strings they produce.

```toml
# pure memo + reconciler: sha3, hex, thiserror — nothing else
tempo-reconcile = { ..., features = [] }

# add CSV/JSON export
tempo-reconcile = { ..., features = ["export"] }

# add HTTP polling watcher (adds tokio + reqwest + rustls)
tempo-reconcile = { ..., features = ["watcher"] }

# add WebSocket push watcher (adds tokio-tungstenite on top of watcher)
tempo-reconcile = { ..., features = ["watcher-ws"] }

# add HMAC-signed webhook delivery
tempo-reconcile = { ..., features = ["webhook"] }

# add Tempo Explorer REST client
tempo-reconcile = { ..., features = ["explorer"] }
```

`reqwest` uses pure-Rust TLS (`rustls` + `ring`), so there is no OpenSSL dependency.
If you need your own HTTP transport, implement `get_tip20_transfer_history` directly
by calling your client and passing the raw log values to `watcher::decode::decode_log`
(accessible when the `watcher` feature is enabled).

## Modules

- **`memo`** — pure functions, zero I/O: encode/decode bytes32, ULID conversion, issuer tag
- **`reconciler`** — stateful matching engine with pluggable `ReconcileStore` trait
- **`export`** — CSV, JSON, JSONL formatters; HMAC-signed webhook delivery (`feature = "webhook"`)
- **`watcher`** — `eth_getLogs` HTTP polling watcher (`feature = "watcher"`)
- **`watcher-ws`** — WebSocket push watcher with auto-reconnect (`feature = "watcher-ws"`)
- **`explorer`** — Tempo Explorer REST client: metadata, balances, history (`feature = "explorer"`)

## Links

- [Memo spec](../../docs-public/MEMO-SPEC.md)
- [Test vectors](../../spec/vectors.json)
- [TypeScript SDK](../../ts/)
- [Main README](../../README.md)

## License

MIT
