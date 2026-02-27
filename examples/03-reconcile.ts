#!/usr/bin/env npx tsx
/**
 * Example: full reconciliation flow — register expected, fetch history, match, report.
 *
 * Usage: npx tsx examples/03-reconcile.ts
 */
import {
  Reconciler,
  getTip20TransferHistory,
  exportCsv,
  decodeMemo,
} from '../ts/src/index'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'

// Known block with TransferWithMemo activity
const BLOCK = 6504870n

async function main() {
  console.log('Fetching pathUSD transfers from block', BLOCK.toString(), '...\n')

  const events = await getTip20TransferHistory({
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    fromBlock: BLOCK,
    toBlock: BLOCK + 30n,
    batchSize: 10,
  })

  console.log(`Found ${events.length} transfer events\n`)

  const withMemo = events.filter((e) => e.memoRaw)
  console.log(`Events with memo: ${withMemo.length}`)
  for (const e of withMemo.slice(0, 5)) {
    const decoded = e.memoRaw ? decodeMemo(e.memoRaw) : null
    const memoStr = typeof decoded === 'string'
      ? decoded
      : decoded && typeof decoded === 'object'
        ? `v1/${decoded.t}/${decoded.ulid}`
        : '(undecoded)'
    console.log(`  tx=${e.txHash.slice(0, 14)}... amount=${Number(e.amount) / 1e6} memo="${memoStr}"`)
  }

  const reconciler = new Reconciler({ allowPartial: true })

  if (withMemo.length > 0) {
    const sample = withMemo[0]!
    reconciler.expect({
      memoRaw: sample.memoRaw!,
      token: PATH_USD,
      to: sample.to,
      amount: sample.amount,
    })
    console.log(`\nRegistered 1 expected payment (memo=${sample.memoRaw!.slice(0, 20)}...)`)
  }

  const results = reconciler.ingestMany(events)
  console.log(`Ingested ${results.length} events\n`)

  const report = reconciler.report()
  console.log('--- Reconciliation Report ---')
  console.log(`Matched:  ${report.summary.matchedCount}`)
  console.log(`Partial:  ${report.summary.partialCount}`)
  console.log(`Pending:  ${report.summary.pendingCount}`)
  console.log(`No memo:  ${report.summary.noMemoCount}`)
  console.log(`Unknown:  ${report.summary.unknownMemoCount}`)
  console.log(`Amount $:  ${Number(report.summary.totalMatchedAmount) / 1e6} matched`)

  if (report.matched.length > 0) {
    const csv = exportCsv(report.matched)
    console.log('\n--- CSV Output (first 2 lines) ---')
    const lines = csv.split('\n')
    console.log(lines[0])
    if (lines[1]) console.log(lines[1])
  }
}

main().catch(console.error)
