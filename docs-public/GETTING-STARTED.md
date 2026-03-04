# Getting started

## Install

```bash
npm i @tempo-reconcile/sdk
```

Peer dependency: `viem >= 2.0`. If you don't have it:

```bash
npm i viem
```

## 1. Encode a memo

Every payment you expect needs a memo. Generate a ULID, encode it:

```typescript
import { encodeMemoV1, issuerTagFromNamespace } from '@tempo-reconcile/sdk'
import { ulid } from 'ulid' // or any ULID library

const ISSUER = issuerTagFromNamespace('my-app')

const paymentId = ulid() // e.g. "01MASW9NF6YW40J40H289H858P"
const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: ISSUER,
  ulid: paymentId,
})
// memo = "0x01..." (bytes32 hex string)
```

Store the ULID in your database. Share the memo hex with whoever is paying you.

## 2. Send a payment (payer side)

The payer calls `transferWithMemo` on the TIP-20 token contract:

```typescript
import { createWalletClient, http } from 'viem'

await walletClient.writeContract({
  address: '0x20C0000000000000000000000000000000000000', // pathUSD on Moderato
  abi: [{
    name: 'transferWithMemo',
    type: 'function',
    inputs: [
      { name: 'to', type: 'address' },
      { name: 'amount', type: 'uint256' },
      { name: 'memo', type: 'bytes32' },
    ],
    outputs: [],
    stateMutability: 'nonpayable',
  }],
  functionName: 'transferWithMemo',
  args: [recipientAddress, 10_000_000n, memo], // 10 USDC (6 decimals)
})
```

## 3. Watch incoming payments

On the receiving side, watch for transfer events:

```typescript
import { watchTip20Transfers } from '@tempo-reconcile/sdk'

const stop = watchTip20Transfers(
  {
    rpcUrl: 'https://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000',
    to: '0xYourAddress',
  },
  (event) => {
    console.log('Payment received:', event.txHash)
    console.log('Memo:', event.memo?.ulid)
    console.log('Amount:', event.amount)
  }
)

// later: stop()
```

## 4. Reconcile

The reconciler matches incoming payments to expected records:

```typescript
import { Reconciler, encodeMemoV1, issuerTagFromNamespace } from '@tempo-reconcile/sdk'

const ISSUER = issuerTagFromNamespace('my-app')
const reconciler = new Reconciler()

// register what you expect
reconciler.expect({
  memoRaw: encodeMemoV1({ type: 'invoice', issuerTag: ISSUER, ulid: '01MASW9NF6YW40J40H289H858P' }),
  token: '0x20C0000000000000000000000000000000000000',
  to: '0xYourAddress',
  amount: 10_000_000n, // 10 USDC
  meta: { invoiceId: 'INV-2026-001' },
})

// when a payment event comes in:
const result = reconciler.ingest(event)

switch (result.status) {
  case 'matched':
    console.log('Paid:', result.expected?.meta?.invoiceId)
    break
  case 'mismatch_amount':
    console.log('Wrong amount:', result.reason)
    break
  case 'unknown_memo':
    console.log('Unrecognized payment')
    break
}
```

## 5. Export

```typescript
import { exportCsv, exportJson } from '@tempo-reconcile/sdk'

const report = reconciler.report()

// CSV for spreadsheets / ERPs
const csv = exportCsv([...report.matched, ...report.issues])

// JSON for APIs
const json = exportJson(report.matched)
```

## Match statuses

| Status | What happened |
|--------|---------------|
| `matched` | Memo found, amount correct, all good |
| `mismatch_amount` | Memo found, wrong amount |
| `mismatch_token` | Memo found, wrong token contract |
| `mismatch_party` | Memo found, wrong sender or recipient |
| `unknown_memo` | Memo present but not in expected payments |
| `no_memo` | Transfer without memo |
| `expired` | Expected payment was past due |
| `partial` | Partial payment accumulated, not yet complete |

Duplicate events are handled via idempotency: ingesting the same `(txHash, logIndex)` twice returns the cached result silently.

Register expectations before ingesting events. If a payment arrives before its `expect()` call, it gets `unknown_memo` and the result is cached — re-ingesting won't re-evaluate. To reprocess, clear the store with `reconciler.reset()`.

## Rust quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
tempo-reconcile = { version = "0.1", features = ["serde", "export", "watcher"] }
```

### Encode a memo

```rust
use tempo_reconcile::{encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params, MemoType};

let issuer = issuer_tag_from_namespace("my-app");
let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
    memo_type: MemoType::Invoice,
    issuer_tag: issuer,
    ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
    salt: None,
}).unwrap();
// memo_raw = "0x01..." (bytes32 hex)
```

### Reconcile

```rust
use tempo_reconcile::{Reconciler, ReconcilerOptions, ExpectedPayment, PaymentEvent, MatchStatus};

let mut rec = Reconciler::new(ReconcilerOptions::new());
rec.expect(ExpectedPayment {
    memo_raw: memo_raw.clone(),
    token: "0x20C0000000000000000000000000000000000000".into(),
    to: "0xYourAddress".into(),
    amount: 10_000_000, // 10 USDC (6 decimals)
    from: None, due_at: None, meta: None,
}).unwrap();

let result = rec.ingest(PaymentEvent {
    memo_raw: Some(memo_raw), amount: 10_000_000,
    token: "0x20C0000000000000000000000000000000000000".into(),
    to: "0xYourAddress".into(), from: "0xSender".into(),
    chain_id: 42431, block_number: 1, log_index: 0,
    tx_hash: "0xabc".into(), memo: None, timestamp: None,
});
assert_eq!(result.status, MatchStatus::Matched);
```

### Export

```rust
use tempo_reconcile::{export_csv, export_json};

let report = rec.report();
let csv = export_csv(&report.matched);
let json = export_json(&report.matched);
```

See the [Rust API reference](API.md#rust-api-reference) for full documentation and feature flags.

## Error handling

**Memo functions** return `null` (TypeScript) or `None` (Rust) for invalid input.
They never throw.

**Reconciler** `expect()` throws if the same memo is registered twice.
`ingest()` always returns a `MatchResult` — it never throws.

**Watcher (HTTP polling)** throws on startup if the RPC URL is unreachable.
After starting, transient RPC errors are skipped and the poller retries on the
next cycle. 429 responses respect the `Retry-After` header automatically.

**Watcher (WebSocket)** reconnects automatically after disconnections, up to
`maxReconnects` times (default 5; set to 0 to disable reconnection). After
exhausting retries the watcher stops silently — check your handle if uptime matters.

**Export** functions are pure transforms. They do not throw.

## Networks

| Network | Chain ID | RPC | Status |
|---------|----------|-----|--------|
| Moderato testnet | 42431 | `https://rpc.moderato.tempo.xyz` | Active |
| Mainnet | 4217 | TBD | Pending launch |

Tempo Moderato is natively supported in viem. Instead of hardcoding chain IDs:

```typescript
import { tempoModerato } from 'viem/chains'

// use tempoModerato.id instead of 42431
const stop = watchTip20Transfers({
  rpcUrl: 'https://rpc.moderato.tempo.xyz',
  chainId: tempoModerato.id,
  token: '0x20C0000000000000000000000000000000000000',
  to: '0xYourAddress',
}, callback)
```

## Testnet tokens

| Token | Address | Decimals |
|-------|---------|----------|
| pathUSD | `0x20C0000000000000000000000000000000000000` | 6 |
| AlphaUSD | `0x20C0000000000000000000000000000000000001` | 6 |
| BetaUSD | `0x20C0000000000000000000000000000000000002` | 6 |
| ThetaUSD | `0x20C0000000000000000000000000000000000003` | 6 |

Get test tokens:
```bash
cast rpc tempo_fundAddress 0xYourAddress --rpc-url https://rpc.moderato.tempo.xyz
```
