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

**Reconnection:** If the WebSocket connection drops, the watcher automatically reconnects with exponential backoff (1s, 2s, 4s, ... capped at 30s). Set `maxReconnects: 0` to disable. The `onError` callback fires on each failed attempt.

**Note on reorgs:** Tempo has deterministic ~0.5s finality. There are no reorgs, so the watcher does not implement reorg handling. If you use this SDK on a chain with reorgs, you need to handle that yourself.

**Note on timestamps:** The watcher does not populate `event.timestamp` because that would require an extra `getBlock` RPC call per log. If you need timestamps (e.g. for reconciler expiry via `dueAt`), enrich events yourself before calling `reconciler.ingest()`.

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

### `exportCsv(results: MatchResult[]): string`

CSV string. Columns: timestamp, block_number, tx_hash, from, to, token, amount_raw, amount_human, memo_raw, memo_type, memo_ulid, status, expected_amount, reason, and any meta_* columns.

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
  fetch: customFetch,      // optional, for Node <18 or testing
  onBatchError: (err) => console.error(err.error, err.statusCode),
})

result.sent    // number of results delivered
result.failed  // number of results that failed after all retries
result.errors  // WebhookBatchError[] — failed batch details
```

`WebhookBatchError` has the original `results` for the failed batch, plus optional `statusCode` and `error` string.

Headers: `X-Tempo-Reconcile-Idempotency-Key`, `X-Tempo-Reconcile-Timestamp`. If `secret` is set: `X-Tempo-Reconcile-Signature` (HMAC-SHA256).

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
- `getMetadata(address): Promise<AddressMetadata>`
- `getBalances(address): Promise<BalancesResponse>`
- `getHistory(address, opts?): Promise<HistoryResponse>`

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

For production, implement `ReconcileStore` backed by a database. All methods must be synchronous or return a Promise. The interface is small — 10 methods total.

```typescript
import type { ReconcileStore, ExpectedPayment, MatchResult } from '@tempo-reconcile/sdk'

class PostgresStore implements ReconcileStore {
  constructor(private db: YourDbClient) {}

  async addExpected(payment: ExpectedPayment): Promise<void> {
    await this.db.query(
      'INSERT INTO expected_payments (memo_raw, data) VALUES ($1, $2)',
      [payment.memoRaw, JSON.stringify(payment, (_, v) => typeof v === 'bigint' ? v.toString() : v)]
    )
  }

  async getExpected(memoRaw: `0x${string}`): Promise<ExpectedPayment | undefined> {
    const row = await this.db.query('SELECT data FROM expected_payments WHERE memo_raw = $1', [memoRaw])
    return row ? JSON.parse(row.data) : undefined
  }

  // ... implement remaining methods
}

const reconciler = new Reconciler({ store: new PostgresStore(db) })
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
type HistoryOptions = WatchOptions & { fromBlock: bigint; toBlock?: bigint; batchSize?: number }
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
  createdTxHash: `0x${string}`
  createdBy: `0x${string}`
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

## Publishing

Releases are published to npm via the `release.yml` GitHub Actions workflow.

**Setup:**

1. Create a GitHub environment called `npm-publish` in your repository settings
2. Add an `NPM_TOKEN` secret to that environment (generate at npmjs.com > Access Tokens)
3. Tag a commit with `v*` (e.g. `git tag v0.1.0 && git push --tags`)

The workflow validates that the git tag matches `package.json` version, runs tests and lint, builds, and publishes with `--provenance`. It also creates a GitHub Release with auto-generated notes.
