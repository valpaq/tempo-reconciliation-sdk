#!/usr/bin/env npx tsx
/**
 * TIP-20 payment reconciliation — end-to-end showcase.
 *
 * Fetches real transfers from Moderato testnet, registers an expected payment,
 * reconciles, and exports the result as CSV.
 *
 * Usage: npx tsx examples/reconcile-showcase.ts
 */
import {
  getTip20TransferHistory,
  Reconciler,
  decodeMemo,
  isMemoV1,
  exportCsv,
} from '../ts/src/index'

const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'

async function main() {
  // 1. Fetch transfers from a known block range on Moderato testnet
  const events = await getTip20TransferHistory({
    rpcUrl: 'https://rpc.moderato.tempo.xyz',
    chainId: 42431,
    token: PATH_USD,
    fromBlock: 6504870n,
    toBlock: 6504900n,
  })
  console.log(`Fetched ${events.length} transfers`)

  // 2. Decode memos — structured v1 or plain-text
  for (const e of events.filter(e => e.memoRaw).slice(0, 3)) {
    const memo = decodeMemo(e.memoRaw!)
    const label = isMemoV1(memo) ? `v1/${memo.t}/${memo.ulid}` : String(memo)
    console.log(`  ${e.txHash.slice(0, 14)}... ${Number(e.amount) / 1e6} pathUSD  memo="${label}"`)
  }

  // 3. Register expected payment and reconcile
  const reconciler = new Reconciler({ allowPartial: true })
  const withMemo = events.filter(e => e.memoRaw)

  if (withMemo.length > 0) {
    const first = withMemo[0]!
    reconciler.expect({
      memoRaw: first.memoRaw!,
      token: PATH_USD,
      to: first.to,
      amount: first.amount,
      meta: { invoiceId: 'INV-001' },
    })
  }

  reconciler.ingestMany(events)
  const report = reconciler.report()

  console.log(`\nMatched: ${report.summary.matchedCount}  Pending: ${report.summary.pendingCount}  Issues: ${report.summary.issueCount}`)

  // 4. Export matched results as CSV
  if (report.matched.length > 0) {
    const csv = exportCsv(report.matched)
    console.log('\n' + csv.split('\n').slice(0, 2).join('\n'))
  }
}

main().catch(console.error)
