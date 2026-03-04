# Changelog

## [0.1.0] - 2026-02-27

Initial release.

### Memo (TEMPO-RECONCILE-MEMO-001)

- `encodeMemoV1` / `decodeMemoV1` — encode and decode bytes32 memos
- `randomSalt` — generate 7 cryptographically random salt bytes
- `issuerTagFromNamespace` — derive 8-byte namespace tag from string
- `ulidToBytes16` / `bytes16ToUlid` — ULID binary conversion
- 6 payment types: invoice, payroll, refund, batch, subscription, custom

### Watcher

- `watchTip20Transfers` — HTTP polling for TransferWithMemo events
- `watchTip20TransfersWs` — WebSocket subscription (eth_subscribe)
- `getTip20TransferHistory` — batch historical fetch with auto-pagination
- Deduplication, transfer-only mode, error callbacks

### Reconciler

- `Reconciler` class — expect/ingest/report pattern
- Matching by memo, token, recipient, sender (optional strict mode)
- Partial payments, overpayment tolerance, expiry rejection
- `InMemoryStore` default, pluggable `ReconcileStore` interface

### Export

- `exportCsv` — RFC 4180 CSV with meta columns
- `exportJson` / `exportJsonl` — JSON and newline-delimited JSON
- `sendWebhook` — batched delivery with HMAC signing, exponential backoff retry, error reporting via `onBatchError`

### Explorer

- `createExplorerClient` — Tempo Explorer REST API (metadata, balances, history)

### Rust crate (`tempo-reconcile`)

- Full API parity with the TypeScript SDK
- `encode_memo_v1` / `decode_memo_v1` / `decode_memo` — bytes32 memo encode/decode
- `Reconciler` with all 8 `MatchStatus` variants, partial payments, tolerance, expiry
- `export_csv` / `export_json` / `export_jsonl` — formatted output (feature `export`)
- `send_webhook` — HMAC-signed HTTP delivery with jitter backoff (feature `webhook`)
- `watch_tip20_transfers` — HTTP polling watcher (feature `watcher`)
- `watch_tip20_transfers_ws` — WebSocket watcher (feature `watcher-ws`)
- `ExplorerClient` — Tempo Explorer HTTP client (feature `explorer`)
- CLI binary `tempo-reconcile` — `memo encode/decode/generate/issuer-tag`, `run`
