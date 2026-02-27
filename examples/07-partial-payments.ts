#!/usr/bin/env npx tsx
/**
 * Example: partial payment reconciliation.
 * An invoice for 100 pathUSD is paid in 3 installments: 40 + 35 + 25.
 * The reconciler tracks cumulative progress until the invoice is fully paid.
 *
 * Usage: npx tsx examples/07-partial-payments.ts
 */
import {
  Reconciler,
  encodeMemoV1,
  issuerTagFromNamespace,
} from '../ts/src/index'
import type { PaymentEvent } from '../ts/src/index'

const TOKEN: `0x${string}` = '0x20C0000000000000000000000000000000000000'
const RECIPIENT: `0x${string}` = '0x2222222222222222222222222222222222222222'
const SENDER: `0x${string}` = '0x1111111111111111111111111111111111111111'

const tag = issuerTagFromNamespace('billing-app')
const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: tag,
  ulid: '01MASW9NF6YW40J40H289H858P',
})

console.log('--- Partial Payment Example ---\n')
console.log(`Invoice memo: ${memo.slice(0, 20)}...`)
console.log(`Expected amount: 100.00 pathUSD (100_000_000 units)\n`)

const reconciler = new Reconciler({ allowPartial: true })

reconciler.expect({
  memoRaw: memo,
  token: TOKEN,
  to: RECIPIENT,
  amount: 100_000_000n,  // 100 pathUSD (6 decimals)
})

// Simulate 3 partial payments: 40 + 35 + 25 = 100
const payments: [bigint, string][] = [
  [40_000_000n, 'Payment 1: 40 pathUSD'],
  [35_000_000n, 'Payment 2: 35 pathUSD'],
  [25_000_000n, 'Payment 3: 25 pathUSD'],
]

for (let i = 0; i < payments.length; i++) {
  const [amount, label] = payments[i]!
  const event: PaymentEvent = {
    chainId: 42431,
    blockNumber: BigInt(6504870 + i),
    txHash: `0x${'ab'.repeat(16)}${i.toString(16).padStart(32, '0')}` as `0x${string}`,
    logIndex: 0,
    token: TOKEN,
    from: SENDER,
    to: RECIPIENT,
    amount,
    memoRaw: memo,
  }

  const result = reconciler.ingest(event)
  console.log(`${label}`)
  console.log(`  status: ${result.status}`)
  if (result.remainingAmount !== undefined) {
    console.log(`  remaining: ${Number(result.remainingAmount) / 1e6} pathUSD`)
  }
  if (result.status === 'matched') {
    console.log(`  fully paid!`)
  }
  console.log()
}

const report = reconciler.report()
console.log('--- Report ---')
console.log(`Matched:  ${report.summary.matchedCount}`)
console.log(`Partial:  ${report.summary.partialCount}`)
console.log(`Pending:  ${report.summary.pendingCount}`)
