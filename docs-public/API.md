# API reference

`@tempo-reconcile/sdk`

## memo

### `issuerTagFromNamespace(namespace: string): bigint`

Compute issuer tag from a namespace string. Returns uint64 as bigint.

```typescript
issuerTagFromNamespace('tempo-reconcile') // -> 18193562290988123368n
```

### `encodeMemoV1(params): \`0x${string}\``

Pack memo fields into a bytes32 hex string.

```typescript
const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: issuerTagFromNamespace('my-app'),
  ulid: '01MASW9NF6YW40J40H289H858P',
  salt: new Uint8Array(7), // optional, defaults to zeros
})
```

| Param | Type | Required | Default |
|-------|------|----------|---------|
| `type` | `MemoType` | yes | -- |
| `issuerTag` | `bigint` | yes | -- |
| `ulid` | `string` | yes | -- |
| `salt` | `Uint8Array \| 'random'` | no | 7 zero bytes |

Pass `salt: 'random'` to generate 7 cryptographically random bytes. Throws if ULID is not 26 chars, type is invalid, or salt is not 7 bytes.

### `randomSalt(): Uint8Array`

Generate 7 cryptographically random bytes for the salt field. Uses `crypto.getRandomValues`.

### `decodeMemoV1(memoRaw): MemoV1 | null`

Decode a bytes32 hex string as structured v1 memo. Returns null if it doesn't match v1 format.

```typescript
const parsed = decodeMemoV1('0x01fc7c8482914a04e8...')
// { v: 1, t: 'invoice', issuerTag: 18193562290988123368n, ulid: '01MASW...', id16: Uint8Array, salt: Uint8Array, raw: '0x...' }

decodeMemoV1('0x00000000...') // null (type 0x00 is reserved)
```

Never throws. Safe on any input.

### `decodeMemoText(memoRaw): string | null`

Decode a bytes32 hex string as UTF-8 text. Handles both right-padded and left-padded memos.

```typescript
decodeMemoText('0x5041592d353935303739...00') // -> "PAY-595079" (right-padded)
decodeMemoText('0x0000...64696e6e6572303031') // -> "dinner001"  (left-padded)
decodeMemoText('0x0000...0000')                // -> null
```

Never throws. Returns null for all-zeros or non-UTF-8 input.

### `decodeMemo(memoRaw): MemoV1 | string | null`

Unified decoder: tries v1 structured format first, then falls back to UTF-8 text.

```typescript
decodeMemo(encodedV1Memo) // -> { v: 1, t: 'invoice', ... }
decodeMemo(textMemoHex)   // -> "PAY-595079"
decodeMemo(allZeros)      // -> null
```

### `isMemoV1(memo): memo is MemoV1`

Type guard: narrows a `decodeMemo()` result to `MemoV1`.

```typescript
const memo = decodeMemo(event.memoRaw)
if (isMemoV1(memo)) {
  console.log(memo.ulid) // TypeScript knows this is MemoV1
}
```

### `ulidToBytes16(ulid: string): Uint8Array`

Convert a 26-char Crockford Base32 ULID string to its 16-byte binary representation. Throws if the ULID is not exactly 26 characters.

```typescript
const bytes = ulidToBytes16('01MASW9NF6YW40J40H289H858P')
// Uint8Array(16) [ 1, 141, ... ]
```

| Param | Type | Description |
|-------|------|-------------|
| `ulid` | `string` | 26-char Crockford Base32 ULID |

**Returns:** `Uint8Array` (16 bytes)
**Throws:** If `ulid` is not exactly 26 characters

### `bytes16ToUlid(id16: Uint8Array): string`

Convert a 16-byte binary array back to a 26-char Crockford Base32 ULID string. Throws if the input is not exactly 16 bytes.

```typescript
const ulid = bytes16ToUlid(new Uint8Array([1, 141, /* ... 14 more bytes */]))
// '01MASW9NF6YW40J40H289H858P'
```

| Param | Type | Description |
|-------|------|-------------|
| `id16` | `Uint8Array` | 16-byte ULID binary |

**Returns:** `string` (26-char ULID)
**Throws:** If `id16` is not exactly 16 bytes

---

## watcher

### `watchTip20Transfers(options, callback): () => void`

Watch for TIP-20 transfers in real time via HTTP polling. Returns an unsubscribe function.

```typescript
const stop = watchTip20Transfers(
  {
    rpcUrl: 'https://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000',
    to: '0xMyAddress',
  },
  (event) => { /* PaymentEvent */ }
)
```

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `rpcUrl` | `string` | yes | -- | HTTP RPC endpoint |
| `chainId` | `number` | yes | -- | 42431 (Moderato) or 4217 (mainnet) |
| `token` | `` `0x${string}` `` | yes | -- | TIP-20 token address |
| `to` | `` `0x${string}` `` | no | -- | Filter by recipient |
| `from` | `` `0x${string}` `` | no | -- | Filter by sender |
| `startBlock` | `bigint` | no | latest | Start from this block |
| `pollIntervalMs` | `number` | no | 1000 | Polling interval |
| `dedupeTtlMs` | `number` | no | 60000 | Dedup cache TTL |
| `includeTransferOnly` | `boolean` | no | false | Also emit Transfer events (no memo) |
| `onError` | `(err: Error) => void` | no | -- | Error callback |

Events are deduplicated by `(txHash, logIndex)`.

### `watchTip20TransfersWs(options, callback): () => void`

Watch for TIP-20 transfers via WebSocket push subscription. No polling. Events are deduplicated by `(txHash, logIndex)`.

```typescript
const stop = watchTip20TransfersWs(
  {
    wsUrl: 'wss://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: '0x20C0000000000000000000000000000000000000',
  },
  (event) => { /* PaymentEvent */ }
)
```

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `wsUrl` | `string` | yes | -- | WebSocket RPC endpoint |
| `chainId` | `number` | yes | -- | Chain ID |
| `token` | `` `0x${string}` `` | yes | -- | TIP-20 token address |
| `to` | `` `0x${string}` `` | no | -- | Filter by recipient |
| `from` | `` `0x${string}` `` | no | -- | Filter by sender |
| `dedupeTtlMs` | `number` | no | 60000 | Dedup cache TTL |
| `includeTransferOnly` | `boolean` | no | false | Also emit Transfer events |
| `onError` | `(err: Error) => void` | no | -- | Error callback |
| `maxReconnects` | `number` | no | 5 | Max reconnection attempts after WebSocket drops. 0 = no reconnect |
| `reconnectDelayMs` | `number` | no | 1000 | Base delay (ms) before first reconnect. Doubles each attempt, capped at 30s |

Reconnection: If the WebSocket connection drops, the watcher reconnects with exponential backoff (1s, 2s, 4s... capped at 30s). Set `maxReconnects: 0` to disable. `onError` fires on each failed attempt.

**Rust SDK callback difference:** The Rust watcher uses a batch callback `Fn(Vec<PaymentEvent>)` instead of per-event `Fn(PaymentEvent)`. This is idiomatic for Rust and avoids repeated mutex locks. When porting code between SDKs, wrap the Rust callback with `for event in events { ... }`.

Note on reorgs: Tempo has deterministic ~0.5s finality with no reorgs, so the watcher doesn't implement reorg handling. If you use this on a chain with reorgs, handle it yourself.

Note on timestamps: The watcher doesn't populate `event.timestamp` — that would require an extra `getBlock` call per log. Enrich events yourself before `reconciler.ingest()` if you need timestamps for `dueAt`.

### `getTip20TransferHistory(options): Promise<PaymentEvent[]>`

Fetch historical transfers in a block range.

```typescript
const events = await getTip20TransferHistory({
  rpcUrl: 'https://rpc.moderato.tempo.xyz',
  chainId: 42431,
  token: '0x20C0000000000000000000000000000000000000',
  to: '0xMyAddress',
  fromBlock: 1000n,
  toBlock: 2000n,
})
```

Same options as `watchTip20Transfers` plus `fromBlock` (required), `toBlock` (optional, default latest), `batchSize` (optional, default 2000 blocks per getLogs call).

---

## reconciler

### `new Reconciler(options?)`

```typescript
const reconciler = new Reconciler({
  issuerTag: issuerTagFromNamespace('my-app'), // only match this issuer
  strictSender: false,      // require sender match (default false)
  allowOverpayment: true,   // accept overpayments (default true)
  rejectExpired: false,     // reject payments after dueAt (default false)
  amountToleranceBps: 0,    // basis points tolerance (default 0 = exact)
  allowPartial: false,      // accept partial payments (default false)
  store: new InMemoryStore(), // custom persistence (default: in-memory)
})
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `issuerTag` | `bigint` | -- | Only match memos with this issuer tag |
| `strictSender` | `boolean` | false | Require sender address match |
| `allowOverpayment` | `boolean` | true | Accept overpayments as matched |
| `rejectExpired` | `boolean` | false | Reject payments past dueAt |
| `amountToleranceBps` | `number` | 0 | Basis points tolerance (50 = 0.5%) |
| `allowPartial` | `boolean` | false | Track and accumulate partial payments |
| `partialToleranceMode` | `'final' \| 'each'` | `'final'` | How tolerance interacts with partial payments |
| `store` | `ReconcileStore` | `InMemoryStore` | Persistence backend |

**Tolerance modes:** In `"final"` mode, partial payments accumulate and tolerance applies to the cumulative total. In `"each"` mode, there is no accumulation — each payment must independently match within tolerance. The `partial` status is only emitted in `"final"` mode.

### `reconciler.expect(payment)`

Register an expected payment. `memoRaw` is the primary key.

```typescript
reconciler.expect({
  memoRaw: '0x01fc7c8482...',
  token: '0x20C0000000000000000000000000000000000000',
  to: '0xMyAddress',
  amount: 10_000_000n,
  from: '0xSender',          // optional
  dueAt: 1709200000,         // optional, unix seconds
  meta: { invoiceId: 'INV-001' }, // optional, arbitrary strings
})
```

Throws if memoRaw is already registered.

### `reconciler.ingest(event): MatchResult`

Process one incoming PaymentEvent. Returns a MatchResult.

Idempotent: ingesting the same event twice returns the cached result.

**Note:** If an event arrives before its `expect()` call, it is cached as `unknown_memo`. Re-ingesting after `expect()` returns the cached result, not a re-evaluation. Use `reconciler.reset()` to clear and reprocess.

### `reconciler.ingestMany(events): MatchResult[]`

Process multiple events. Returns results in the same order.

### `reconciler.report(): ReconcileReport`

```typescript
const report = reconciler.report()
report.matched   // MatchResult[] -- status === 'matched'
report.issues    // MatchResult[] -- anything else
report.pending   // ExpectedPayment[] -- not yet received
report.summary   // { totalExpected, matchedCount, pendingCount, partialCount, ... }
```

### `reconciler.removeExpected(memoRaw): boolean`

Remove an expected payment. Returns true if it existed.

### `reconciler.reset()`

Clear everything. Start fresh.

---

## export

> **Dependency note:** The `export` module uses the Web Crypto API (`crypto.subtle`) for computing webhook idempotency keys (SHA-256) and HMAC signatures. No additional dependencies required — `crypto.subtle` is available in Node.js 20+, Deno, and Cloudflare Workers.

### `exportCsv(results: MatchResult[]): string`

CSV string with 23 fixed columns plus dynamic `meta_*` columns (one per unique key in `expected.meta` across all results):

```
timestamp, block_number, tx_hash, log_index, chain_id,
from, to, token, amount_raw, amount_human,
memo_raw, memo_type, memo_ulid, memo_issuer_tag,
status, expected_amount, expected_from, expected_to, expected_due_at,
reason, overpaid_by, is_late, remaining_amount,
meta_<key> ...
```

### `exportJson(results: MatchResult[]): string`

JSON string, pretty-printed.

### `exportJsonl(results: MatchResult[]): string`

One JSON object per line.

### `sendWebhook(options): Promise<WebhookResult>`

POST match results to an HTTP endpoint. Retries on 5xx, 429, 408, and network errors with exponential backoff (1s, 2s, 4s, capped at 30s). No retry on other 4xx.

```typescript
const result = await sendWebhook({
  url: 'https://my-backend.com/hooks/reconcile',
  results: report.matched,
  secret: 'whsec_...',    // optional, HMAC-SHA256 signature
  batchSize: 50,           // default 50
  maxRetries: 3,           // default 3
  timeoutMs: 30_000,       // default 30s, per-batch request timeout
  fetch: customFetch,      // optional, for testing or custom environments
  onBatchError: (err) => console.error(err.error, err.statusCode),
})

result.sent    // number of results delivered
result.failed  // number of results that failed after all retries
result.errors  // WebhookBatchError[] — failed batch details
```

`WebhookBatchError` has the original `results` for the failed batch, plus optional `statusCode` and `error` string.

### `sign(payload, secret): Promise<string>`

Compute HMAC-SHA256 signature for webhook payload verification. Uses the Web Crypto API (`globalThis.crypto.subtle`), available in Node.js 20+, Deno, Cloudflare Workers, and modern browsers.

```typescript
import { sign } from '@tempo-reconcile/sdk'

const signature = await sign(requestBody, 'whsec_...')
if (signature !== request.headers['x-tempo-reconcile-signature']) {
  throw new Error('Invalid signature')
}
```

| Param | Type | Description |
|-------|------|-------------|
| `payload` | `string` | The raw request body string |
| `secret` | `string` | The shared HMAC secret |

**Returns:** Lowercase hex-encoded HMAC-SHA256 signature.

Headers sent on every request:

| Header | Value |
|--------|-------|
| `X-Tempo-Reconcile-Timestamp` | Unix seconds at batch creation time |
| `X-Tempo-Reconcile-Idempotency-Key` | SHA-256(body) hex — stable across retries for the same batch |
| `X-Tempo-Reconcile-Signature` | HMAC-SHA256(body, secret) hex — only present when `secret` is set |

Body shape:

```json
{
  "id": "whevt_<32-hex-chars>",
  "timestamp": 1709123456,
  "events": [
    {
      "status": "matched",
      "payment": {
        "chainId": 42431, "blockNumber": "100", "txHash": "0x...", "logIndex": 0,
        "token": "0x20c0...", "from": "0xsender", "to": "0xrecipient",
        "amount": "10000000", "memoRaw": "0x01..."
      },
      "expected": {
        "amount": "10000000",
        "meta": { "invoiceId": "INV-001" }
      },
      "reason": null,
      "overpaidBy": null,
      "remainingAmount": null,
      "isLate": false
    }
  ]
}
```

All `amount` fields are decimal strings (safe for large values).

---

## explorer

### `ExplorerClient` / `createExplorerClient(options?)`

Client for the Tempo Explorer REST API. Use `createExplorerClient()` as a factory or instantiate `new ExplorerClient(options)` directly.

```typescript
const explorer = createExplorerClient()
// equivalent: new ExplorerClient()
// or: createExplorerClient({ baseUrl: 'https://custom-explorer.com/api' })

const meta = await explorer.getMetadata('0x51881fed...')
const balances = await explorer.getBalances('0x51881fed...')
const history = await explorer.getHistory('0x51881fed...', { limit: 50 })
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `baseUrl` | `string` | `https://explore.tempo.xyz/api` | Explorer API base URL |
| `fetch` | `typeof globalThis.fetch` | `globalThis.fetch` | Custom fetch for Node <18 or testing |

Methods:
- `getMetadata(address): Promise<AddressMetadata>` — account type, tx count, creation info
- `getBalances(address): Promise<BalancesResponse>` — TIP-20 token balances
- `getHistory(address, opts?): Promise<HistoryResponse>` — paginated tx list with parsed events

```typescript
// getMetadata
const meta = await explorer.getMetadata('0x51881fed...')
// { address: '0x...', chainId: 42431, accountType: 'eoa', txCount: 42, ... }

// getBalances
const { balances } = await explorer.getBalances('0x51881fed...')
// [{ token: '0x20C0...', balance: '50000000', symbol: 'pathUSD', decimals: 6, ... }]

// getHistory (paginated)
const page = await explorer.getHistory('0x51881fed...', { limit: 20, offset: 0 })
// { transactions: [...], total: 142, hasMore: true, ... }
```

---

## Storage

### `ReconcileStore` (interface)

Interface for persistence. Implement this to back the Reconciler with a database.

```typescript
type ReconcileStore = {
  addExpected(payment: ExpectedPayment): void
  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined
  getAllExpected(): ExpectedPayment[]
  removeExpected(memoRaw: `0x${string}`): boolean
  addResult(key: string, result: MatchResult): void
  getResult(key: string): MatchResult | undefined
  getAllResults(): MatchResult[]
  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint
  getPartialTotal(memoRaw: `0x${string}`): bigint
  removePartial(memoRaw: `0x${string}`): void
  clear(): void
}
```

### `InMemoryStore`

Default in-memory implementation. Suitable for scripts and tests. State is lost on process restart.

```typescript
import { InMemoryStore } from '@tempo-reconcile/sdk'

const store = new InMemoryStore()
const reconciler = new Reconciler({ store })
```

### Custom store (database)

For production, implement `ReconcileStore` backed by a database. **All methods must be synchronous** — the Reconciler calls them without `await`. The interface is small: 10 methods.

```typescript
import type { ReconcileStore, ExpectedPayment, MatchResult } from '@tempo-reconcile/sdk'

class SqliteStore implements ReconcileStore {
  constructor(private db: YourSyncDbClient) {}

  addExpected(payment: ExpectedPayment): void {
    this.db.run(
      'INSERT INTO expected_payments (memo_raw, data) VALUES (?, ?)',
      [payment.memoRaw, JSON.stringify(payment, (_, v) => typeof v === 'bigint' ? v.toString() : v)]
    )
  }

  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined {
    const row = this.db.get('SELECT data FROM expected_payments WHERE memo_raw = ?', [memoRaw])
    return row ? JSON.parse(row.data) : undefined
  }

  // ... implement remaining methods
}

const reconciler = new Reconciler({ store: new SqliteStore(db) })
```

**Notes:**
- Store `bigint` fields (`amount`, `issuerTag`) as `TEXT` or `NUMERIC` in SQL — JSON loses precision on large integers
- The key for results is `${txHash}:${logIndex}` — index on this column
- `addPartial` must be atomic (use `INSERT ... ON CONFLICT DO UPDATE` or a transaction)
- `removePartial` is called after a partial payment series reaches the expected amount — clean up your accumulation row

---

## Types

```typescript
type MemoType = 'invoice' | 'payroll' | 'refund' | 'batch' | 'subscription' | 'custom'

type MemoV1 = {
  v: 1
  t: MemoType
  issuerTag: bigint
  ulid: string
  id16: Uint8Array
  salt: Uint8Array
  raw: `0x${string}`
}

type PaymentEvent = {
  chainId: number
  blockNumber: bigint
  txHash: `0x${string}`
  logIndex: number
  token: `0x${string}`
  from: `0x${string}`
  to: `0x${string}`
  amount: bigint
  memoRaw?: `0x${string}`
  memo?: MemoV1 | string | null
  timestamp?: number
}

type ExpectedPayment = {
  memoRaw: `0x${string}`
  token: `0x${string}`
  to: `0x${string}`
  amount: bigint
  from?: `0x${string}`
  dueAt?: number
  meta?: Record<string, string>
}

type MatchStatus =
  | 'matched'
  | 'partial'
  | 'unknown_memo'
  | 'no_memo'
  | 'mismatch_amount'
  | 'mismatch_token'
  | 'mismatch_party'
  | 'expired'

type MatchResult = {
  status: MatchStatus
  payment: PaymentEvent
  expected?: ExpectedPayment
  reason?: string
  overpaidBy?: bigint
  remainingAmount?: bigint
  isLate?: boolean
}

type ReconcileSummary = {
  totalExpected: number
  totalReceived: number
  matchedCount: number
  issueCount: number
  pendingCount: number
  totalExpectedAmount: bigint
  totalReceivedAmount: bigint
  totalMatchedAmount: bigint
  unknownMemoCount: number
  noMemoCount: number
  mismatchAmountCount: number
  mismatchTokenCount: number
  mismatchPartyCount: number
  expiredCount: number
  partialCount: number
}

// Options types (documented inline above in each function section)
type EncodeMemoV1Params = { type: MemoType; issuerTag: bigint; ulid: string; salt?: Uint8Array | 'random' }
type WatchOptions = { rpcUrl: string; chainId: number; token: `0x${string}`; to?: `0x${string}`; from?: `0x${string}`; startBlock?: bigint; pollIntervalMs?: number; dedupeTtlMs?: number; includeTransferOnly?: boolean; onError?: (err: Error) => void }
type HistoryOptions = { rpcUrl: string; chainId: number; token: `0x${string}`; to?: `0x${string}`; from?: `0x${string}`; includeTransferOnly?: boolean; onError?: (err: Error) => void; fromBlock: bigint; toBlock?: bigint; batchSize?: number }
type WatchWsOptions = { wsUrl: string; chainId: number; token: `0x${string}`; to?: `0x${string}`; from?: `0x${string}`; includeTransferOnly?: boolean; dedupeTtlMs?: number; onError?: (err: Error) => void; maxReconnects?: number; reconnectDelayMs?: number }
type ReconcilerOptions = { store?: ReconcileStore; issuerTag?: bigint; strictSender?: boolean; allowOverpayment?: boolean; rejectExpired?: boolean; amountToleranceBps?: number; allowPartial?: boolean; partialToleranceMode?: 'final' | 'each' }
type WebhookOptions = { url: string; results: MatchResult[]; secret?: string; batchSize?: number; maxRetries?: number; timeoutMs?: number; fetch?: typeof globalThis.fetch; onBatchError?: (err: WebhookBatchError) => void }
type ExplorerOptions = { baseUrl?: string; fetch?: typeof globalThis.fetch }

type ReconcileReport = {
  matched: MatchResult[]
  issues: MatchResult[]
  pending: ExpectedPayment[]
  summary: ReconcileSummary
}

type WebhookResult = { sent: number; failed: number; errors: WebhookBatchError[] }
type WebhookBatchError = { results: MatchResult[]; statusCode?: number; error?: string }

// Explorer types
type AddressMetadata = {
  address: `0x${string}`
  chainId: number
  accountType: string
  txCount: number
  lastActivityTimestamp: number
  createdTimestamp: number
  createdTxHash?: `0x${string}`
  createdBy?: `0x${string}`
}

type TokenBalance = { token: `0x${string}`; balance: string; name: string; symbol: string; currency: string; decimals: number }

type BalancesResponse = { balances: TokenBalance[] }

type KnownEventPart = { type: 'action' | 'amount' | 'text' | 'account'; value: string | { token: string; value: string; decimals: number; symbol: string } }
type KnownEvent = { type: string; note?: string; parts: KnownEventPart[]; meta?: Record<string, string> }
type ExplorerTransaction = { hash: `0x${string}`; blockNumber: string; timestamp: number; from: `0x${string}`; to: `0x${string}`; value: string; status: string; gasUsed: string; effectiveGasPrice: string; knownEvents: KnownEvent[] }

type HistoryResponse = {
  transactions: ExplorerTransaction[]
  total: number
  offset: number
  limit: number
  hasMore: boolean
  countCapped: boolean
  error: string | null
}
```

---

## Rust API reference

The `tempo-reconcile` crate mirrors the TypeScript SDK. Add it to `Cargo.toml`:

```toml
[dependencies]
tempo-reconcile = { version = "0.1", features = ["serde", "rand", "export", "watcher", "webhook", "explorer"] }
```

### Feature flags

| Feature | What it enables | Extra deps |
|---------|----------------|------------|
| *(none)* | memo encode/decode, reconciler | — |
| `rand` | `random_salt()` | `rand` |
| `serde` | `Serialize`/`Deserialize` on all types | `serde` |
| `export` | `export_csv`, `export_json`, `export_jsonl` | `serde_json` |
| `webhook` | `send_webhook` | `reqwest`, `hmac`, `sha2`, `rand` |
| `watcher` | `watch_tip20_transfers`, `get_tip20_transfer_history` | `reqwest`, `tokio` |
| `watcher-ws` | `watch_tip20_transfers_ws` | `tokio-tungstenite` |
| `explorer` | `ExplorerClient` | `reqwest` |

Typical production setup:

```toml
tempo-reconcile = { version = "0.1", features = ["serde", "export", "watcher", "webhook"] }
```

### Memo functions

| Function | Description |
|---|---|
| `encode_memo_v1(params: &EncodeMemoV1Params)` | Encode a bytes32 memo. Returns `Result<String, MemoError>`. |
| `decode_memo_v1(hex: &str)` | Decode a v1 structured memo. Returns `Option<MemoV1>`. |
| `decode_memo(hex: &str)` | Decode any memo — v1 or plain text. Returns `Option<Memo>`. |
| `decode_memo_text(hex: &str)` | Try to decode a bytes32 as UTF-8 text. Returns `Option<String>`. |
| `is_memo_v1(hex: &str)` | Return `true` if the hex string is a valid v1 memo. |
| `issuer_tag_from_namespace(ns: &str)` | Compute the 8-byte issuer tag from a namespace string. |
| `random_salt()` | Generate a random 7-byte salt (requires feature `rand`). |
| `bytes16_to_ulid(id16: &[u8; 16])` | Convert 16-byte binary ULID to a Crockford base32 string. |
| `ulid_to_bytes16(ulid: &str)` | Convert a ULID string to 16-byte binary. |

`EncodeMemoV1Params` struct:
```rust
pub struct EncodeMemoV1Params {
    pub memo_type: MemoType,
    pub issuer_tag: u64,
    pub ulid: String,
    pub salt: Option<[u8; 7]>,
}
```

### Reconciler

`Reconciler::new(opts: ReconcilerOptions)` — creates a reconciler backed by `InMemoryStore`.
`Reconciler::with_store(store: S, opts: ReconcilerOptions)` — creates a reconciler with a custom store.
`ReconcilerOptions::new()` sets `allow_overpayment: true`; all other fields use `Default`.

**`ReconcilerOptions` fields:**

| Field | Type | Default | Description |
|---|---|---|---|
| `issuer_tag` | `Option<u64>` | `None` | Only match memos with this issuer tag. `None` accepts any issuer. |
| `strict_sender` | `bool` | `false` | Require `event.from == expected.from` when `expected.from` is set. |
| `allow_overpayment` | `bool` | `true` | Accept payments above the expected amount as `matched`. |
| `allow_partial` | `bool` | `false` | Accumulate underpayments; emit `matched` when cumulative total reaches expected. |
| `reject_expired` | `bool` | `false` | Emit `expired` for payments arriving after `expected.due_at`. |
| `amount_tolerance_bps` | `u32` | `0` | Basis-point tolerance (100 = 1%). Capped at 10 000. |
| `partial_tolerance_mode` | `ToleranceMode` | `Final` | How tolerance applies to partial payments (see below). |

`ToleranceMode::Final` — tolerance applies to the final cumulative total only.
`ToleranceMode::Each` — tolerance applies per individual payment; a single underpayment beyond tolerance is immediately `mismatch_amount`.

**Match statuses:**

| Status | Description |
|--------|-------------|
| `Matched` | Memo found, amount within tolerance, all checks passed |
| `Partial` | Partial payment accumulated, cumulative total below expected |
| `UnknownMemo` | Memo present but not in expected payments (or issuer tag mismatch) |
| `NoMemo` | Transfer event without a memo field |
| `MismatchAmount` | Memo found but amount outside tolerance |
| `MismatchToken` | Memo found but wrong token contract |
| `MismatchParty` | Memo found but wrong sender (strict mode) or recipient |
| `Expired` | Payment arrived after `due_at` (with `reject_expired` enabled) |

Store types: `ReconcileStore` (trait), `InMemoryStore` (default in-memory implementation).

Errors: `ReconcileError`, `MemoError`.

### Export

| Function | Description |
|---|---|
| `export_csv(results: &[MatchResult])` | Serialize results as CSV (requires feature `export`). |
| `export_json(results: &[MatchResult])` | Serialize results as a JSON array. |
| `export_jsonl(results: &[MatchResult])` | Serialize results as newline-delimited JSON. |

### Watcher (feature `watcher`)

| Function / Type | Description |
|---|---|
| `watch_tip20_transfers(config: WatchConfig, cb)` | Start polling watcher. Returns `WatchHandle` (call `.stop()` to cancel). |
| `get_tip20_transfer_history(config: WatchConfig)` | Fetch historical transfers in one call. |
| `WatchConfig` | RPC URL, chain ID, token, address filters, polling interval. |
| `WatchHandle` | Handle returned by `watch_tip20_transfers`. Call `.stop()` to cancel. |
| `WatcherError` | Error type for watcher operations. |

Websocket variant (feature `watcher-ws`): `watch_tip20_transfers_ws(config: WatchWsConfig, cb)`, `WatchWsConfig`.

### Webhook (feature `webhook`)

| Function / Type | Description |
|---|---|
| `send_webhook(config: WebhookConfig, results: &[MatchResult])` | POST results to a webhook endpoint with retry + jitter. |
| `WebhookConfig` | URL, HMAC secret, batch size, max retries, timeout. |
| `WebhookResult` | `{ sent, failed, errors }` — aggregate delivery outcome. |
| `WebhookBatchError` | Failed batch with optional `status_code` and `error` string. |
| `WebhookError` | Error type for webhook operations. |

### Explorer (feature `explorer`)

| Type / Method | Description |
|---|---|
| `ExplorerClient::new(base_url: &str)` | Create a client pointed at a custom Explorer API URL. |
| `ExplorerClient::get_metadata(addr: &str)` | Fetch `AddressMetadata` for an address. Returns `ExplorerError::NotFound` on 404. |
| `ExplorerClient::get_balances(addr: &str)` | Fetch `BalancesResponse` for an address. Returns empty balances on 404. |
| `ExplorerClient::get_history(addr, limit, offset)` | Fetch `HistoryResponse` (paginated). `limit`/`offset` are `Option<u32>`. Returns empty on 404. |
| `BalancesResponse` | `{ balances: Vec<TokenBalance> }` — wrapper returned by `get_balances`. |
| `HistoryResponse` | Paginated response: `transactions`, `total`, `offset`, `limit`, `has_more`, `count_capped`, `error`. |
| `ExplorerTransaction` | Full transaction: `hash`, `block_number`, `timestamp`, `from`, `to`, `value`, `status`, `gas_used`, `effective_gas_price`, `known_events`. |
| `KnownEvent` | `{ event_type, note, parts: Vec<KnownEventPart>, meta }` |
| `KnownEventPart` | `{ part_type: String, value: KnownEventPartValue }` |
| `KnownEventPartValue` | `Text(String)` or `Amount { token, value, decimals, symbol }` |
| `ExplorerError` | Error type: `NotFound(String)`, `Http(u16)`, `Network(String)`, `Parse(String)`. |

### Rust nonces crate (`tempo-reconcile-nonces`)

```toml
[dependencies]
tempo-reconcile-nonces = "0.1"
```

Uses [alloy](https://docs.rs/alloy) for RPC and types (`Address`, `U256`, `FixedBytes<32>`).

#### `NoncePool`

```rust
use tempo_reconcile_nonces::{NoncePool, NoncePoolOptions, NonceMode};
use alloy::primitives::address;

let mut pool = NoncePool::new(NoncePoolOptions {
    address: address!("1234567890abcdef1234567890abcdef12345678"),
    rpc_url: "https://rpc.moderato.tempo.xyz".into(),
    lanes: 4,
    mode: NonceMode::Lanes,
    ..Default::default()
})?;
pool.init().await?;
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(opts: NoncePoolOptions) -> Result<Self, NonceError>` | Create a pool. Validates options. |
| `chain_id` | `(&self) -> u64` | Configured chain ID. |
| `init` | `(&mut self) -> Result<(), NonceError>` | Query on-chain nonces, populate slots. Must call before `acquire()`. |
| `acquire` | `(&mut self, request_id: Option<&str>) -> Result<&NonceSlot, NonceError>` | Reserve next free slot. Auto-reaps stale slots first. Idempotent on `request_id`. |
| `submit` | `(&mut self, nonce_key: U256, tx_hash: FixedBytes<32>) -> Result<(), NonceError>` | Mark reserved slot as submitted. |
| `confirm` | `(&mut self, nonce_key: U256) -> Result<(), NonceError>` | Confirm submitted slot. Increments nonce, resets to free. |
| `fail` | `(&mut self, nonce_key: U256) -> Result<(), NonceError>` | Fail a submitted or reserved slot. Resets to free, same nonce (not consumed). |
| `release` | `(&mut self, nonce_key: U256) -> Result<(), NonceError>` | Release slot back to free regardless of state. |
| `reap` | `(&mut self) -> Vec<NonceSlot>` | Reclaim slots past `reservation_ttl_ms`. Called automatically by `acquire()`. |
| `slots` | `(&self) -> &[NonceSlot]` | Immutable view of all slots. |
| `stats` | `(&self) -> NoncePoolStats` | Aggregate counts by state. |
| `reset` | `(&mut self) -> Result<(), NonceError>` | Re-query all on-chain nonces, reset all slots to free. |

#### `NoncePoolOptions`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `address` | `Address` | required | Sender account address |
| `rpc_url` | `String` | required | RPC endpoint URL |
| `lanes` | `u32` | `4` | Parallel lanes (lanes mode only) |
| `mode` | `NonceMode` | `Lanes` | `Lanes` or `Expiring` |
| `reservation_ttl_ms` | `u64` | `30_000` | Auto-expire stale reservations (ms) |
| `valid_before_offset_s` | `u64` | `30` | Expiring mode: `validBefore = now + offset` (seconds) |
| `chain_id` | `u64` | `42431` | Chain ID (Moderato testnet) |
| `validate_chain_id` | `bool` | `false` | Validate chain ID against RPC at init |

#### `NonceSlot`

| Field | Type | Description |
|-------|------|-------------|
| `nonce_key` | `U256` | Lane key (1..N for lanes, `U256::MAX` for expiring) |
| `nonce` | `u64` | Current sequence value |
| `state` | `SlotState` | `Free`, `Reserved`, `Submitted` |
| `reserved_at` | `Option<Instant>` | When reserved |
| `submitted_at` | `Option<Instant>` | When submitted |
| `tx_hash` | `Option<FixedBytes<32>>` | Transaction hash once submitted |
| `request_id` | `Option<String>` | Caller-provided idempotency key |
| `valid_before` | `Option<u64>` | Unix seconds — tx must be included before this (expiring mode) |

#### `NoncePoolStats`

| Field | Type | Description |
|-------|------|-------------|
| `total` | `usize` | Total managed slots |
| `free` | `usize` | Available slots |
| `reserved` | `usize` | Reserved but not submitted |
| `submitted` | `usize` | Pending on-chain confirmation |
| `confirmed` | `u64` | Cumulative confirmed count |
| `failed` | `u64` | Cumulative failed count |
| `expired` | `u64` | Cumulative reaped/expired count |

#### Enums

```rust
pub enum NonceMode { Lanes, Expiring }
pub enum SlotState { Free, Reserved, Submitted }
```

#### RPC helpers

| Function | Description |
|----------|-------------|
| `get_nonce_from_precompile<P: Provider>(provider, address, key) -> Result<u64, NonceError>` | Query nonce precompile for a specific (address, key) pair. |
| `get_protocol_nonce<P: Provider>(provider, address) -> Result<u64, NonceError>` | Query protocol nonce via `get_transaction_count`. |

#### Constants

| Name | Type | Value | Description |
|------|------|-------|-------------|
| `NONCE_PRECOMPILE` | `Address` | `0x4e4F4E4345...` | Tempo nonce precompile |
| `MAX_U256` | `U256` | `2^256 - 1` | Nonce key for expiring mode |
| `MODERATO_CHAIN_ID` | `u64` | `42431` | Moderato testnet chain ID |
| `DEFAULT_LANES` | `u32` | `4` | Default parallel lanes |
| `DEFAULT_RESERVATION_TTL_MS` | `u64` | `30_000` | Default reservation TTL (ms) |
| `DEFAULT_VALID_BEFORE_OFFSET_S` | `u64` | `30` | Default validBefore offset (s) |

#### `NonceError`

| Variant | Description |
|---------|-------------|
| `MissingAddress` | Address is required |
| `MissingRpcUrl` | RPC URL is required |
| `InvalidLanes` | Lanes must be >= 1 |
| `InvalidTtl` | Reservation TTL must be > 0 |
| `InvalidValidBefore` | validBefore offset must be > 0 |
| `NotInitialized` | Pool not initialized — call `init()` first |
| `AlreadyInitialized` | Already initialized — call `reset()` to re-sync |
| `Exhausted` | No free slots available |
| `SlotNotFound(U256)` | Slot not found for given nonce key |
| `InvalidState { .. }` | Slot in wrong state for the operation |
| `ChainIdMismatch { .. }` | Configured chain ID != RPC chain ID |
| `Rpc(alloy::contract::Error)` | Underlying RPC/transport error |

---

## `@tempo-reconcile/nonces`

Nonce pool for Tempo's 2D nonce system. Manages parallel transaction lanes and expiring nonces (TIP-1009).

```bash
npm i @tempo-reconcile/nonces
```

### `NoncePool`

```typescript
import { NoncePool } from '@tempo-reconcile/nonces'
```

#### `constructor(options: NoncePoolOptions)`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `address` | `` `0x${string}` `` | required | Sender account address |
| `rpcUrl` | `string` | required | RPC endpoint URL |
| `mode` | `"lanes" \| "expiring"` | `"lanes"` | Concurrency strategy |
| `lanes` | `number` | `4` | Number of parallel lanes (lanes mode only) |
| `reservationTtlMs` | `number` | `30000` | Auto-expire stale reservations after this duration (ms) |
| `validBeforeOffsetS` | `number` | `30` | Expiring mode: `validBefore = now + offset` (seconds). Must be > 0. |
| `chainId` | `number` | `42431` | Chain ID (Moderato testnet) |
| `validateChainId` | `boolean` | `false` | If `true`, `init()` calls `eth_chainId` and throws if the RPC chain ID does not match `chainId`. Disabled by default to avoid an extra round-trip when the endpoint is known-correct. |

Throws if `address` or `rpcUrl` is empty, `lanes < 1`, `reservationTtlMs <= 0`, or `validBeforeOffsetS <= 0`.

#### `init(): Promise<void>`

Query on-chain nonce values and populate slots. Must be called before `acquire()`.

#### `acquire(requestId?: string): NonceSlot`

Reserve the next free slot. Auto-reaps stale reservations before declaring exhaustion.

If `requestId` is provided and a reserved or submitted slot with the same ID exists, returns it (idempotent). After confirm/fail/reap, a new slot is allocated for the same `requestId`.

Throws `"NoncePool: no free slots available"` if no free slots.

#### `submit(nonceKey: bigint, txHash: \`0x${string}\`): void`

Mark a reserved slot as submitted. Throws if slot is not in `"reserved"` state.

#### `confirm(nonceKey: bigint): void`

Mark a submitted slot as confirmed. In lanes mode, increments the nonce and resets the slot to `"free"`. In expiring mode, resets the slot without incrementing (call `reset()` to re-query the on-chain nonce before the next `acquire()`). Throws if slot is not in `"submitted"` state.

#### `fail(nonceKey: bigint): void`

Mark a submitted or reserved slot as failed. Resets to `"free"` with the same nonce value (nonce was not consumed on-chain). Throws if slot is not in `"submitted"` or `"reserved"` state.

#### `release(nonceKey: bigint): void`

Release a slot back to `"free"` regardless of current state.

#### `reap(): NonceSlot[]`

Reclaim slots that have exceeded `reservationTtlMs`. Called automatically at the start of `acquire()`. Returns the reaped slots.

#### `reset(): Promise<void>`

Re-query all on-chain nonce values and reset all slots to `"free"`.

#### `getSlots(): readonly NonceSlot[]`

Readonly view of all slots.

#### `getStats(): NoncePoolStats`

Aggregate counts by state: `total`, `free`, `reserved`, `submitted`, `confirmed`, `failed`, `expired`.

### `NonceSlot`

| Field | Type | Description |
|-------|------|-------------|
| `nonceKey` | `bigint` | Lane key (1..N for lanes, maxUint256 for expiring) |
| `nonce` | `bigint` | Current sequence value |
| `state` | `SlotState` | `"free" \| "reserved" \| "submitted"` |
| `reservedAt` | `number?` | Unix ms when reserved |
| `submittedAt` | `number?` | Unix ms when submitted |
| `txHash` | `` `0x${string}`? `` | Transaction hash once submitted |
| `requestId` | `string?` | Caller-provided idempotency key |
| `validBefore` | `number?` | Unix seconds — tx must be included before this (expiring mode) |

### Constants

| Name | Value | Description |
|------|-------|-------------|
| `NONCE_PRECOMPILE` | `0x4e4F4E4345000000000000000000000000000000` | Address of the Tempo nonce precompile |
| `MAX_UINT256` | `2n**256n - 1n` | `nonceKey` used for expiring mode (TIP-1009) |
| `MODERATO_CHAIN_ID` | `42431` | Chain ID of the Moderato testnet |
| `DEFAULT_LANES` | `4` | Default number of parallel lanes |
| `DEFAULT_RESERVATION_TTL_MS` | `30_000` | Default reservation TTL in milliseconds |
| `DEFAULT_VALID_BEFORE_OFFSET_S` | `30` | Default `validBefore` offset in seconds (expiring mode) |
| `INONCE_ABI` | ABI array | Viem-compatible ABI for the `INonce` precompile (`getNonce(address, uint256) → uint64`) |

### `NonceMode`

```ts
type NonceMode = "lanes" | "expiring";
```

- `"lanes"` — parallel nonces, one per lane key (1..N). Suitable for high-throughput concurrent sends.
- `"expiring"` — TIP-1009 single slot with a `validBefore` deadline. Required for time-bounded transactions.

### `getNonceFromPrecompile(client, address, nonceKey): Promise<bigint>`

Query the nonce precompile (`0x4e4F4E4345000000000000000000000000000000`) for a specific lane.

### `getProtocolNonce(client, address): Promise<bigint>`

Query the standard protocol nonce via `getTransactionCount({ blockTag: "pending" })`.

---

## Publishing

Releases are published via the `release.yml` GitHub Actions workflow.

### TypeScript SDK (`@tempo-reconcile/sdk`)

1. Create a GitHub environment called `npm-publish` with an `NPM_TOKEN` secret
2. Tag a commit with `v*` (e.g. `git tag v0.1.0 && git push --tags`)
3. The workflow validates the tag matches `package.json` version, runs tests, builds, and publishes with `--provenance`

### TypeScript Nonces (`@tempo-reconcile/nonces`)

1. Tag a commit with `nonces-v*` (e.g. `git tag nonces-v0.1.0 && git push --tags`)
2. Same validation and publish flow as the SDK, targeting `ts/packages/nonces/`

### Rust crate (`tempo-reconcile`)

1. Add a `CARGO_REGISTRY_TOKEN` secret to the `crates-publish` environment
2. Tag triggers `cargo publish -p tempo-reconcile`
3. CLI binaries are built for Linux (x86_64, aarch64) and macOS (x86_64, aarch64) and attached to the GitHub Release
