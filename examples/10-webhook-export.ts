#!/usr/bin/env npx tsx
/**
 * Example: export reconciliation results in CSV, JSON, and JSONL formats.
 * Also shows what a webhook payload looks like.
 *
 * Usage: npx tsx examples/10-webhook-export.ts
 */
import {
  encodeMemoV1,
  issuerTagFromNamespace,
  exportCsv,
  exportJson,
  exportJsonl,
} from '../ts/src/index'
import type { MatchResult } from '../ts/src/index'

const TOKEN: `0x${string}` = '0x20C0000000000000000000000000000000000000'
const tag = issuerTagFromNamespace('shop')

const memo1 = encodeMemoV1({ type: 'invoice', issuerTag: tag, ulid: '01MASW9NF6YW40J40H289H858P' })
const memo2 = encodeMemoV1({ type: 'refund', issuerTag: tag, ulid: '01MASW9NF6YW40J40H289H999Z' })

const results: MatchResult[] = [
  {
    status: 'matched',
    payment: {
      chainId: 42431,
      blockNumber: 6504870n,
      txHash: '0xba01fd25c190087f10d6d6d921f2d4a3e0e7aafd21e92cbb7f56851060e3d3ba' as `0x${string}`,
      logIndex: 0,
      token: TOKEN,
      from: '0x1111111111111111111111111111111111111111' as `0x${string}`,
      to: '0x2222222222222222222222222222222222222222' as `0x${string}`,
      amount: 50_000_000n,
      memoRaw: memo1,
      timestamp: 1700000000,
    },
    expected: {
      memoRaw: memo1,
      token: TOKEN,
      to: '0x2222222222222222222222222222222222222222' as `0x${string}`,
      amount: 50_000_000n,
      meta: { orderId: 'ORD-001', customer: 'alice' },
    },
  },
  {
    status: 'matched',
    payment: {
      chainId: 42431,
      blockNumber: 6504875n,
      txHash: '0xcc02fd25c190087f10d6d6d921f2d4a3e0e7aafd21e92cbb7f56851060e3d3cc' as `0x${string}`,
      logIndex: 1,
      token: TOKEN,
      from: '0x3333333333333333333333333333333333333333' as `0x${string}`,
      to: '0x2222222222222222222222222222222222222222' as `0x${string}`,
      amount: 25_000_000n,
      memoRaw: memo2,
      timestamp: 1700001000,
    },
    expected: {
      memoRaw: memo2,
      token: TOKEN,
      to: '0x2222222222222222222222222222222222222222' as `0x${string}`,
      amount: 25_000_000n,
      meta: { orderId: 'ORD-002', customer: 'bob' },
    },
  },
]

console.log('=== CSV ===\n')
const csv = exportCsv(results)
const csvLines = csv.split('\n')
for (const line of csvLines.slice(0, 3)) {
  console.log(line)
}
console.log(`(${csvLines.length - 1} rows total)\n`)

console.log('=== JSON (first result) ===\n')
const json = exportJson(results.slice(0, 1))
const jsonLines = json.split('\n')
for (const line of jsonLines.slice(0, 20)) {
  console.log(line)
}
if (jsonLines.length > 20) console.log('  ...')
console.log()

console.log('=== JSONL ===\n')
const jsonl = exportJsonl(results)
const jsonlLines = jsonl.split('\n').filter(l => l.length > 0)
for (const line of jsonlLines) {
  console.log(line.slice(0, 100) + (line.length > 100 ? '...' : ''))
}
console.log()

console.log('=== Webhook Payload Structure ===\n')
console.log('POST https://your-server.com/webhook')
console.log('Headers:')
console.log('  Content-Type: application/json')
console.log('  X-Tempo-Reconcile-Idempotency-Key: <uuid>')
console.log('  X-Tempo-Reconcile-Timestamp: <unix-seconds>')
console.log('  X-Tempo-Reconcile-Signature: <hmac-sha256-hex> (if secret set)')
console.log('\nBody:')
const samplePayload = {
  id: 'whevt_<uuid>',
  timestamp: Math.floor(Date.now() / 1000),
  events: results.slice(0, 1).map(e => ({
    status: e.status,
    payment: {
      txHash: e.payment.txHash,
      amount: e.payment.amount.toString(),
      from: e.payment.from,
      to: e.payment.to,
      token: e.payment.token,
      blockNumber: e.payment.blockNumber.toString(),
    },
    expected: e.expected ? {
      amount: e.expected.amount.toString(),
      meta: e.expected.meta,
    } : undefined,
  })),
}
console.log(JSON.stringify(samplePayload, null, 2))
