#!/usr/bin/env npx tsx
/**
 * Dumps all TransferWithMemo events from a block range to CSV.
 * Usage: npx tsx analysis/dump-memos.ts [rangeSize] [outputFile]
 *   npx tsx analysis/dump-memos.ts                  # last 100k blocks
 *   npx tsx analysis/dump-memos.ts 1209600          # ~1 week
 *   npx tsx analysis/dump-memos.ts 1209600 week.csv
 */
import {
  getTip20TransferHistory,
  decodeMemoV1,
  decodeMemoText,
  decodeMemo,
} from '../ts/src/index'
import { writeFileSync } from 'fs'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'

async function getBlockNumber(): Promise<bigint> {
  const res = await fetch(RPC_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_blockNumber', params: [] }),
  })
  const json = await res.json() as { result: string }
  return BigInt(json.result)
}

function escCsv(s: string): string {
  if (s.includes(',') || s.includes('"') || s.includes('\n')) {
    return '"' + s.replace(/"/g, '""') + '"'
  }
  return s
}

async function main() {
  const head = await getBlockNumber()
  const rangeSize = process.argv[2] ? Number(process.argv[2]) : 100_000
  const outFile = process.argv[3] ?? 'analysis/memos.csv'

  const fromBlock = head - BigInt(rangeSize)
  const toBlock = head

  console.log(`Chain head: ${head}`)
  console.log(`Scanning ${rangeSize} blocks: ${fromBlock} .. ${toBlock}`)
  console.log(`At ~0.5s/block that's ~${(rangeSize * 0.5 / 3600).toFixed(1)} hours`)
  console.log(`Output: ${outFile}\n`)

  const BATCH = 2000
  const totalBatches = Math.ceil(rangeSize / BATCH)
  let eventCount = 0

  const header = [
    'block_number', 'tx_hash', 'log_index',
    'from', 'to', 'amount_raw', 'amount_human',
    'memo_raw', 'memo_format', 'memo_decoded',
    'memo_v1_type', 'memo_v1_ulid', 'memo_v1_issuer_tag',
  ].join(',')

  const rows: string[] = [header]

  for (let batch = 0; batch < totalBatches; batch++) {
    const batchFrom = fromBlock + BigInt(batch * BATCH)
    const batchTo = batchFrom + BigInt(BATCH) - 1n > toBlock
      ? toBlock
      : batchFrom + BigInt(BATCH) - 1n

    try {
      const events = await getTip20TransferHistory({
        rpcUrl: RPC_URL,
        chainId: CHAIN_ID,
        token: PATH_USD,
        fromBlock: batchFrom,
        toBlock: batchTo,
        batchSize: BATCH,
      })

      for (const e of events) {
        if (!e.memoRaw) continue

        const decoded = decodeMemo(e.memoRaw)
        let format = 'binary'
        let decodedStr = ''
        let v1Type = ''
        let v1Ulid = ''
        let v1IssuerTag = ''

        if (decoded && typeof decoded === 'object') {
          format = 'v1'
          decodedStr = `${decoded.t}/${decoded.ulid}`
          v1Type = decoded.t
          v1Ulid = decoded.ulid
          v1IssuerTag = `0x${decoded.issuerTag.toString(16).padStart(16, '0')}`
        } else if (typeof decoded === 'string') {
          format = 'text'
          decodedStr = decoded
        } else {
          decodedStr = e.memoRaw.slice(0, 20) + '...'
        }

        const amountHuman = (Number(e.amount) / 1e6).toFixed(6)

        rows.push([
          e.blockNumber.toString(),
          e.txHash,
          e.logIndex.toString(),
          e.from,
          e.to,
          e.amount.toString(),
          amountHuman,
          e.memoRaw,
          format,
          escCsv(decodedStr),
          v1Type,
          v1Ulid,
          v1IssuerTag,
        ].join(','))

        eventCount++
      }
    } catch (err) {
      console.error(`  batch ${batch + 1}/${totalBatches} ERROR:`, err)
      // continue with next batch
    }

    if ((batch + 1) % 50 === 0 || batch === totalBatches - 1) {
      const pct = ((batch + 1) / totalBatches * 100).toFixed(1)
      console.log(`  ${batch + 1}/${totalBatches} batches (${pct}%) — ${eventCount} events so far`)
    }
  }

  writeFileSync(outFile, rows.join('\n') + '\n')
  console.log(`\nDone. ${eventCount} events written to ${outFile}`)
}

main().catch(console.error)
