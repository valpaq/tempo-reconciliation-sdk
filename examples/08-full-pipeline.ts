#!/usr/bin/env npx tsx
/**
 * Example: complete reconciliation pipeline.
 * 1. Check address balances via Explorer API
 * 2. Fetch transfer history from chain
 * 3. Register expected payments and reconcile
 * 4. Export results to CSV and JSONL
 *
 * Usage: npx tsx examples/08-full-pipeline.ts
 */
import {
  createExplorerClient,
  getTip20TransferHistory,
  decodeMemo,
  Reconciler,
  exportCsv,
  exportJsonl,
} from '../ts/src/index'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'
const ADDRESS = '0x51881fed631dae3f998dad2cf0c13e0a932cbb11'
const BLOCK = 6504870n

async function main() {
  console.log('=== Step 1: Check Balances ===\n')
  const explorer = createExplorerClient()
  const { balances } = await explorer.getBalances(ADDRESS)
  for (const b of balances.slice(0, 3)) {
    console.log(`  ${b.symbol}: ${(Number(b.balance) / 10 ** b.decimals).toFixed(2)}`)
  }

  console.log('\n=== Step 2: Fetch Transfer History ===\n')
  const events = await getTip20TransferHistory({
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    fromBlock: BLOCK,
    toBlock: BLOCK + 30n,
    batchSize: 10,
  })
  console.log(`Found ${events.length} transfer events`)

  const withMemo = events.filter(e => e.memoRaw)
  console.log(`  with memo: ${withMemo.length}`)
  console.log(`  without memo: ${events.length - withMemo.length}`)

  for (const e of withMemo.slice(0, 3)) {
    const decoded = decodeMemo(e.memoRaw!)
    const memoStr = typeof decoded === 'string'
      ? decoded
      : decoded && typeof decoded === 'object'
        ? `v1/${decoded.t}/${decoded.ulid}`
        : '(undecoded)'
    console.log(`  tx=${e.txHash.slice(0, 14)}... amount=${Number(e.amount) / 1e6} memo="${memoStr}"`)
  }

  console.log('\n=== Step 3: Reconcile ===\n')
  const reconciler = new Reconciler({ allowPartial: true })

  if (withMemo.length > 0) {
    const sample = withMemo[0]!
    reconciler.expect({
      memoRaw: sample.memoRaw!,
      token: PATH_USD,
      to: sample.to,
      amount: sample.amount,
    })
    console.log(`Registered 1 expected payment`)
  }

  const results = reconciler.ingestMany(events)
  const report = reconciler.report()

  console.log(`\nReconciliation results:`)
  console.log(`  Matched:  ${report.summary.matchedCount}`)
  console.log(`  Partial:  ${report.summary.partialCount}`)
  console.log(`  Pending:  ${report.summary.pendingCount}`)
  console.log(`  No memo:  ${report.summary.noMemoCount}`)
  console.log(`  Unknown:  ${report.summary.unknownMemoCount}`)

  console.log('\n=== Step 4: Export ===\n')

  if (report.matched.length > 0) {
    const csv = exportCsv(report.matched)
    console.log('CSV (first 2 lines):')
    const csvLines = csv.split('\n')
    console.log(`  ${csvLines[0]}`)
    if (csvLines[1]) console.log(`  ${csvLines[1]}`)

    console.log('\nJSONL (first entry):')
    const jsonl = exportJsonl(report.matched)
    const jsonlLines = jsonl.split('\n')
    if (jsonlLines[0]) console.log(`  ${jsonlLines[0].slice(0, 120)}...`)
  } else {
    console.log('No matched results to export')
  }

  console.log(`\nPipeline complete. Processed ${results.length} events.`)
}

main().catch(console.error)
