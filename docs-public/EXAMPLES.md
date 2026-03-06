# Examples

## Invoice reconciliation

The most common use case. You send invoices, customers pay on-chain, you match payments to invoices.

```typescript
import {
  issuerTagFromNamespace,
  encodeMemoV1,
  watchTip20Transfers,
  Reconciler,
  exportCsv,
} from '@tempo-reconcile/sdk'
import { ulid } from 'ulid'
import fs from 'fs'

const ISSUER = issuerTagFromNamespace('my-billing-app')
const TOKEN = '0x20C0000000000000000000000000000000000000' as const
const MY_ADDR = '0xYourAddress' as const

const reconciler = new Reconciler({ issuerTag: ISSUER })

// register invoices
const invoices = [
  { id: 'INV-001', amount: 100_000_000n, customer: 'Acme' },
  { id: 'INV-002', amount: 250_000_000n, customer: 'Globex' },
]

for (const inv of invoices) {
  const paymentUlid = ulid()
  const memoRaw = encodeMemoV1({ type: 'invoice', issuerTag: ISSUER, ulid: paymentUlid })

  reconciler.expect({
    memoRaw,
    token: TOKEN,
    to: MY_ADDR,
    amount: inv.amount,
    meta: { invoiceId: inv.id, customer: inv.customer },
  })

  // send this memo to the customer in their payment instructions
  console.log(`${inv.id}: pay with memo ${memoRaw}`)
}

// watch for payments
const stop = watchTip20Transfers(
  { rpcUrl: 'https://rpc.moderato.tempo.xyz', chainId: 42431, token: TOKEN, to: MY_ADDR },
  (event) => {
    const result = reconciler.ingest(event)
    if (result.status === 'matched') {
      console.log(`PAID: ${result.expected?.meta?.invoiceId}`)
    }
  }
)

// periodic CSV export
setInterval(() => {
  const report = reconciler.report()
  fs.writeFileSync('report.csv', exportCsv([...report.matched, ...report.issues]))
}, 60_000)
```

## Batch payouts

You're sending payroll or marketplace payouts to many recipients. Track each one.

```typescript
import { encodeMemoV1, issuerTagFromNamespace, Reconciler } from '@tempo-reconcile/sdk'
import { ulid } from 'ulid'

const ISSUER = issuerTagFromNamespace('payroll-app')
const reconciler = new Reconciler()

const employees = [
  { name: 'Alice', wallet: '0xAlice', salary: 5_000_000_000n },
  { name: 'Bob', wallet: '0xBob', salary: 4_500_000_000n },
]

// for each employee, create a memo and expected payment
const payouts = employees.map((emp) => {
  const paymentUlid = ulid()
  const memoRaw = encodeMemoV1({ type: 'payroll', issuerTag: ISSUER, ulid: paymentUlid })

  reconciler.expect({
    memoRaw,
    token: '0x20C0000000000000000000000000000000000000',
    to: emp.wallet as `0x${string}`,
    amount: emp.salary,
    meta: { employee: emp.name },
  })

  return { ...emp, memoRaw, ulid: paymentUlid }
})

// send the actual transactions (your code)
// ...

// after sending, watch for confirmations and reconcile
// result.status will be 'matched' when each payout lands
```

## Decode arbitrary memos

If you just want to inspect memos from chain, without reconciliation:

```typescript
import { decodeMemoV1, getTip20TransferHistory } from '@tempo-reconcile/sdk'

const events = await getTip20TransferHistory({
  rpcUrl: 'https://rpc.moderato.tempo.xyz',
  chainId: 42431,
  token: '0x20C0000000000000000000000000000000000000',
  fromBlock: 0n,
  toBlock: 1000n,
})

for (const event of events) {
  if (event.memoRaw) {
    const decoded = decodeMemoV1(event.memoRaw)
    if (decoded) {
      console.log(`Block ${event.blockNumber}: ${decoded.t} / ${decoded.ulid} from ${event.from}`)
    } else {
      console.log(`Block ${event.blockNumber}: unknown memo format`)
    }
  }
}
```

## Tolerance and partial payments

Real-world payments don't always arrive as a single exact transfer. This example covers:
- **Amount tolerance**: accept payments within ±0.5% of the expected amount
- **Partial payments**: a single invoice paid across multiple transfers

```typescript
import { Reconciler, encodeMemoV1, issuerTagFromNamespace } from '@tempo-reconcile/sdk'
import { ulid } from 'ulid'

const ISSUER = issuerTagFromNamespace('billing-app')
const TOKEN = '0x20C0000000000000000000000000000000000000' as const
const MY_ADDR = '0xYourAddress' as const

// ── 1. Tolerance: accept up to 0.5% short ───────────────────────────────
const strictReconciler = new Reconciler({
  amountToleranceBps: 50, // 50 basis points = 0.5%
})

const memo1 = encodeMemoV1({ type: 'invoice', issuerTag: ISSUER, ulid: ulid() })
strictReconciler.expect({
  memoRaw: memo1,
  token: TOKEN,
  to: MY_ADDR,
  amount: 10_000_000n, // 10 USDC expected
})

// A payment of 9.95 USDC is within 0.5% tolerance → matched
const result1 = strictReconciler.ingest({
  chainId: 42431, blockNumber: 100n, txHash: '0xaaa...', logIndex: 0,
  token: TOKEN, from: '0xPayer', to: MY_ADDR, amount: 9_950_000n, memoRaw: memo1,
})
console.log(result1.status) // 'matched'

// ── 2. Partial payments: multiple transfers for one invoice ──────────────
const partialReconciler = new Reconciler({
  allowPartial: true,
  amountToleranceBps: 100, // 1% tolerance on the cumulative total
  // partialToleranceMode defaults to 'final': tolerance checked on cumulative, not per-payment
})

const memo2 = encodeMemoV1({ type: 'invoice', issuerTag: ISSUER, ulid: ulid() })
partialReconciler.expect({
  memoRaw: memo2,
  token: TOKEN,
  to: MY_ADDR,
  amount: 100_000_000n, // 100 USDC
})

// First payment: 40 USDC → partial
const p1 = partialReconciler.ingest({
  chainId: 42431, blockNumber: 101n, txHash: '0xbbb...', logIndex: 0,
  token: TOKEN, from: '0xPayer', to: MY_ADDR, amount: 40_000_000n, memoRaw: memo2,
})
console.log(p1.status)          // 'partial'
console.log(p1.remainingAmount) // 60_000_000n

// Second payment: 59.5 USDC → 99.5% cumulative, within 1% tolerance → matched
const p2 = partialReconciler.ingest({
  chainId: 42431, blockNumber: 102n, txHash: '0xccc...', logIndex: 0,
  token: TOKEN, from: '0xPayer', to: MY_ADDR, amount: 59_500_000n, memoRaw: memo2,
})
console.log(p2.status) // 'matched'
```

**Tolerance modes for partial payments:**

| `partialToleranceMode` | When tolerance is checked |
|------------------------|--------------------------|
| `'final'` (default) | On the cumulative total only — individual partials can be any size |
| `'each'` | On every individual payment — each transfer must be within tolerance of the full expected amount |

## WebSocket watcher

For lower latency, use the WebSocket watcher. Same callback API, different connection.

```typescript
import { watchTip20TransfersWs, Reconciler } from '@tempo-reconcile/sdk'

const reconciler = new Reconciler({ issuerTag: ISSUER })

const stop = watchTip20TransfersWs(
  {
    wsUrl: 'wss://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000',
    to: '0xYourAddress',
  },
  (event) => {
    const result = reconciler.ingest(event)
    console.log(result.status, result.payment.txHash)
  }
)

process.on('SIGINT', () => stop())
```

Use HTTP polling (`watchTip20Transfers`) for broad node compatibility. Use WebSocket when you want sub-second delivery and your RPC node supports `eth_subscribe`.

### Reconnection

The WebSocket watcher reconnects automatically after disconnection. Control it with `maxReconnects` (default 5, 0 = no reconnect):

```typescript
const stop = watchTip20TransfersWs(
  {
    wsUrl: 'wss://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000',
    to: '0xYourAddress',
    maxReconnects: 10,        // retry up to 10 times
    reconnectDelayMs: 2000,   // start with 2 s, doubles each attempt, capped at 30 s
    onError: (err) => console.error('watcher error:', err.message),
  },
  (event) => reconciler.ingest(event)
)
```

In Rust (`watcher-ws` feature), the equivalent fields on `WatchWsConfig` are `max_reconnects` and `reconnect_delay_ms`. After exhausting all reconnects, the watcher task silently stops — check the `WatchHandle` join future or add logging in `on_error`.

## Custom ReconcileStore

The default store is in-memory and resets on process restart. For persistence, implement `ReconcileStore`.

```typescript
import { Reconciler } from '@tempo-reconcile/sdk'
import type { ReconcileStore, ExpectedPayment, MatchResult } from '@tempo-reconcile/sdk'

// Write-through pattern: sync access via in-memory Maps, async writes to Postgres in the background.
class PostgresStore implements ReconcileStore {
  private expected = new Map<string, ExpectedPayment>()
  private results = new Map<string, MatchResult>()
  private partials = new Map<string, bigint>()

  constructor(private db: PostgresClient) {}

  addExpected(payment: ExpectedPayment): void {
    if (this.expected.has(payment.memoRaw)) throw new Error(`duplicate: ${payment.memoRaw}`)
    this.expected.set(payment.memoRaw, payment)
    void this.db.query(
      'INSERT INTO expected_payments (memo_raw, token, to_addr, amount) VALUES ($1,$2,$3,$4)',
      [payment.memoRaw, payment.token, payment.to, payment.amount.toString()]
    )
  }

  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined {
    return this.expected.get(memoRaw)
  }

  getAllExpected(): ExpectedPayment[] { return [...this.expected.values()] }

  removeExpected(memoRaw: `0x${string}`): boolean {
    const existed = this.expected.delete(memoRaw)
    if (existed) void this.db.query('DELETE FROM expected_payments WHERE memo_raw = $1', [memoRaw])
    return existed
  }

  addResult(key: string, result: MatchResult): void {
    this.results.set(key, result)
    void this.db.query(
      'INSERT INTO match_results (key, tx_hash, status) VALUES ($1,$2,$3) ON CONFLICT (key) DO UPDATE SET status = $3',
      [key, result.payment.txHash, result.status]
    )
  }

  getResult(key: string): MatchResult | undefined { return this.results.get(key) }
  getAllResults(): MatchResult[] { return [...this.results.values()] }

  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint {
    const total = (this.partials.get(memoRaw) ?? 0n) + amount
    this.partials.set(memoRaw, total)
    return total
  }

  getPartialTotal(memoRaw: `0x${string}`): bigint { return this.partials.get(memoRaw) ?? 0n }
  removePartial(memoRaw: `0x${string}`): void { this.partials.delete(memoRaw) }
  clear(): void { this.expected.clear(); this.results.clear(); this.partials.clear() }
}

const store = new PostgresStore(pgClient)
const reconciler = new Reconciler({ store })
```

See `ReconcileStore` in [API.md](./API.md) for the full method list.

## Webhook integration

Forward matched payments to your backend:

```typescript
import { Reconciler, sendWebhook } from '@tempo-reconcile/sdk'

// after reconciling...
const report = reconciler.report()

await sendWebhook({
  url: 'https://my-backend.com/api/reconcile-webhook',
  results: report.matched,
  secret: process.env.WEBHOOK_SECRET,
  batchSize: 10,
  maxRetries: 3,
})
```

Your backend receives:
```json
{
  "id": "whevt_01MASW...",
  "timestamp": 1709123456,
  "events": [
    {
      "status": "matched",
      "payment": { "txHash": "0x...", "amount": "10000000", "from": "0x..." },
      "expected": { "meta": { "invoiceId": "INV-001" } }
    }
  ]
}
```

Verify with `X-Tempo-Reconcile-Signature` header (HMAC-SHA256).

### Handling webhook errors

`sendWebhook` never throws — failed batches are collected so you can retry or alert without crashing the process.

```typescript
import { sendWebhook } from '@tempo-reconcile/sdk'

const out = await sendWebhook({
  url: 'https://my-backend.com/api/reconcile-webhook',
  results: report.matched,
  secret: process.env.WEBHOOK_SECRET,
  maxRetries: 3,
  onBatchError: (err) => {
    // called once per failed batch, after all retries are exhausted
    console.error(`Batch failed (${err.results.length} events): ${err.error}`)
    if (err.statusCode === 400) {
      // permanent failure — log to your alerting system
      alerting.send(`Webhook rejected: ${err.error}`)
    }
  },
})

console.log(`sent=${out.sent} failed=${out.failed}`)
if (out.failed > 0) {
  for (const e of out.errors) {
    console.error(
      `  ${e.results.length} events, status=${e.statusCode ?? 'network'}: ${e.error}`
    )
  }
}
```

## Explorer — address metadata and balances

**TypeScript:**

```typescript
import { createExplorerClient } from '@tempo-reconcile/sdk'

const client = createExplorerClient()

const meta = await client.getMetadata('0xYourAddress')
console.log(`type: ${meta.accountType}, txs: ${meta.txCount}`)

const { balances } = await client.getBalances('0xYourAddress')
for (const b of balances) {
  console.log(`${b.token}: ${b.balance}`)
}

const { transactions } = await client.getHistory('0xYourAddress', { limit: 50 })
console.log(`${transactions.length} transfers found`)
```

**Rust** (requires `explorer` feature in `Cargo.toml`):

```rust
use tempo_reconcile::ExplorerClient;

let client = ExplorerClient::new("https://explorer.moderato.tempo.xyz");

let meta = client.get_metadata("0xYourAddress").await?;
println!("type: {}, txs: {}", meta.account_type, meta.tx_count);

let balances = client.get_balances("0xYourAddress").await?;
for b in &balances.balances {
    println!("{}: {}", b.token, b.balance);
}

let transfers = client.get_history("0xYourAddress", None, None).await?;
println!("{} transfers found", transfers.transactions.len());
```

## Utility functions

### Random salt

```typescript
import { encodeMemoV1, issuerTagFromNamespace, randomSalt } from '@tempo-reconcile/sdk'

const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: issuerTagFromNamespace('my-app'),
  ulid: '01MASW9NF6YW40J40H289H858P',
  salt: randomSalt(), // 7 random bytes for privacy
})
```

### ULID binary conversion

```typescript
import { ulidToBytes16, bytes16ToUlid } from '@tempo-reconcile/sdk'

const bytes = ulidToBytes16('01MASW9NF6YW40J40H289H858P') // Uint8Array(16)
const ulid = bytes16ToUlid(bytes) // "01MASW9NF6YW40J40H289H858P"
```

### Webhook signature verification

```typescript
import { sign } from '@tempo-reconcile/sdk'

// On your webhook endpoint, verify the signature:
const payload = JSON.stringify(requestBody)
const expected = await sign(payload, process.env.WEBHOOK_SECRET!)
const received = request.headers['x-tempo-reconcile-signature']

if (expected !== received) {
  throw new Error('Invalid webhook signature')
}
```
