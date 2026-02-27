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
