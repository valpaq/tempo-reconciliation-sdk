# tempo-reconcile

Drop-in reconciliation for TIP-20 payments on Tempo: structured bytes32 memos + transfer watcher + invoice matching + CSV/JSON/webhook export.

[![CI](https://github.com/valpaq/tempo-reconciliation-sdk/actions/workflows/ci.yml/badge.svg)](https://github.com/valpaq/tempo-reconciliation-sdk/actions/workflows/ci.yml)
[![Tempo Moderato](https://img.shields.io/badge/Tempo-Moderato_testnet-blue)](https://docs.tempo.xyz)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

---

TIP-20 tokens on Tempo have a native `bytes32` memo field (`transferWithMemo`). This SDK covers the receiving side: watch for payments, decode memos (structured v1 or plain-text), match them to invoices, and export results.

- `TEMPO-RECONCILE-MEMO-001` -- namespaced bytes32 memo layout. No PII on-chain.
- `@tempo-reconcile/sdk` -- TypeScript SDK: memo codec, payment watcher, reconciler, exporters
- `@tempo-reconcile/nonces` -- TypeScript nonce pool for Tempo's 2D nonce system (lanes + expiring)
- `tempo-reconcile` -- Rust crate: same spec + test vectors, idiomatic Rust API

Does not overlap with the official Tempo SDK (no wallet, no signing, no sponsored TX).

## Requirements

Node.js >= 20.0.0 (for native `fetch`). The `sendWebhook` and `createExplorerClient` functions accept an optional `fetch` parameter for environments without global `fetch`.

## Install

**TypeScript / Node.js**
```bash
npm i @tempo-reconcile/sdk
# or
pnpm add @tempo-reconcile/sdk
```

**Rust**
```toml
[dependencies]
tempo-reconcile = "0.1"
```

## Quick start

### Encode a memo

```typescript
import { encodeMemoV1, issuerTagFromNamespace } from '@tempo-reconcile/sdk'

const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: issuerTagFromNamespace('my-company'),
  ulid: '01MASW9NF6YW40J40H289H858P',
})
// -> 0x01... (bytes32 hex)
```

### Send a payment with memo (viem)

```typescript
await walletClient.writeContract({
  address: '0x20C0000000000000000000000000000000000000', // pathUSD
  abi: [{ name: 'transferWithMemo', type: 'function',
          inputs: [{ name: 'to', type: 'address' },
                   { name: 'amount', type: 'uint256' },
                   { name: 'memo', type: 'bytes32' }],
          outputs: [], stateMutability: 'nonpayable' }],
  functionName: 'transferWithMemo',
  args: ['0xRecipient', 10_000_000n, memo], // 10 USDC (6 decimals)
})
```

### Watch and reconcile

```typescript
import {
  watchTip20Transfers,
  Reconciler,
  encodeMemoV1,
  issuerTagFromNamespace,
  exportCsv,
} from '@tempo-reconcile/sdk'

const ISSUER = issuerTagFromNamespace('my-company')
const reconciler = new Reconciler()

// register expected invoice
const memoRaw = encodeMemoV1({ type: 'invoice', issuerTag: ISSUER, ulid: '01MASW9NF6YW40J40H289H858P' })
reconciler.expect({
  memoRaw,
  token: '0x20C0000000000000000000000000000000000000',
  to: '0xMyAddress',
  amount: 10_000_000n,
  meta: { invoiceId: 'INV-001' },
})

// watch and match
const stop = watchTip20Transfers(
  { rpcUrl: 'https://rpc.moderato.tempo.xyz', chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000', to: '0xMyAddress' },
  (event) => {
    const result = reconciler.ingest(event)
    console.log(result.status, result.expected?.meta?.invoiceId)
  }
)

// export
const { matched, issues } = reconciler.report()
console.log(exportCsv([...matched, ...issues]))
```

### Nonce management

```typescript
import { NoncePool } from '@tempo-reconcile/nonces'

const pool = new NoncePool({
  address: '0xYourAddress',
  rpcUrl: 'https://rpc.moderato.tempo.xyz',
})
await pool.init()

const slot = pool.acquire('payment-123')
// Use slot.nonceKey and slot.nonce in your transaction
pool.submit(slot.nonceKey, txHash)
pool.confirm(slot.nonceKey)
```

Register expectations before ingesting events. If a payment arrives before its `expect()` call, it gets `unknown_memo` and the result is cached — re-ingesting won't re-evaluate. To reprocess, clear the store with `reconciler.reset()`.

## Rust

```toml
[dependencies]
tempo-reconcile = "0.1"
```

```rust
use tempo_reconcile::{
    encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params,
    ExpectedPayment, MatchStatus, MemoType, PaymentEvent, Reconciler, ReconcilerOptions,
};

let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
    memo_type: MemoType::Invoice,
    issuer_tag: issuer_tag_from_namespace("my-company"),
    ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
    salt: None,
}).unwrap();

let mut rec = Reconciler::new(ReconcilerOptions::new()).unwrap();
rec.expect(ExpectedPayment {
    memo_raw: memo_raw.clone(),
    token: "0x20C0000000000000000000000000000000000000".to_string(),
    to: "0xMyAddress".to_string(), amount: 10_000_000, // 10 USDC (6 decimals)
    from: None, due_at: None, meta: None,
}).unwrap();

let result = rec.ingest(PaymentEvent {
    memo_raw: Some(memo_raw), amount: 10_000_000,
    token: "0x20C0000000000000000000000000000000000000".to_string(),
    to: "0xMyAddress".to_string(), from: "0xSender".to_string(),
    chain_id: 42431, block_number: 1, log_index: 0,
    tx_hash: "0xabc".to_string(), memo: None, timestamp: None,
});
assert_eq!(result.status, MatchStatus::Matched);
```

For watcher and exporter: [rs/tempo-reconcile/](rs/tempo-reconcile/)

## Memo layout (TEMPO-RECONCILE-MEMO-001)

```
byte 0:      type code (0x01-0x0F = v1 types)
bytes 1-8:   issuerTag (uint64 BE) = first8bytes(keccak256(namespace))
bytes 9-24:  ULID binary (16 bytes)
bytes 25-31: salt (7 bytes, optional, zeros by default)
```

Types: `invoice` (0x1), `payroll` (0x2), `refund` (0x3), `batch` (0x4), `subscription` (0x5), `custom` (0xF)

Memo is a reference. Invoice details, customer data -- all of that lives off-chain.

Full spec: [MEMO-SPEC.md](docs-public/MEMO-SPEC.md)

## Architecture

```
Tempo Chain ──> Watcher ──> Reconciler ──> Exporter
                  |              |             |
             RPC logs      Match by       CSV / JSON
             Decode memo   memo key       Webhook
```

- `memo/` -- encode/decode bytes32 memos
- `watcher/` -- subscribe to TransferWithMemo events
- `reconciler/` -- match payments to expected records
- `export/` -- CSV, JSON, JSONL, webhook

No server. No vendor lock-in. Works standalone with just an RPC URL.

See the [examples directory](ts/packages/sdk/examples/) for runnable scripts covering every module.

## Network

| Network | Chain ID | RPC |
|---------|----------|-----|
| Moderato (testnet) | 42431 | `https://rpc.moderato.tempo.xyz` |
| Mainnet | 4217 | TBD |

Testnet tokens: pathUSD (`0x20C0...0000`), AlphaUSD (`...0001`), BetaUSD (`...0002`), ThetaUSD (`...0003`). All 6 decimals.

## Documentation

- [Getting started](docs-public/GETTING-STARTED.md) -- 5-step tutorial from install to export
- [API reference](docs-public/API.md) -- all exported functions, types, and options
- [Examples & walkthroughs](docs-public/EXAMPLES.md) -- module-by-module usage with both TS and Rust
- [Memo spec](docs-public/MEMO-SPEC.md) -- TEMPO-RECONCILE-MEMO-001 bytes32 layout
- [CLI reference](docs-public/CLI.md) -- `tempo-reconcile` command-line tool (Rust)
- [TS examples](ts/packages/sdk/examples/) -- numbered examples covering every module

## Contributing

PRs welcome. New memo types, better matching rules, more exporters (QuickBooks, Xero), examples.

## License

MIT
