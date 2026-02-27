#!/usr/bin/env npx tsx
/**
 * Incremental TransferWithMemo scanner.
 *
 * First run:  scans from block 0 to chain head, writes CSV + saves last block.
 * Next runs:  resumes from saved block, appends new events to existing CSV.
 *
 * Usage:
 *   npx tsx analysis/scan-incremental.ts                        # default files
 *   npx tsx analysis/scan-incremental.ts --out memos-all.csv    # custom CSV path
 *   npx tsx analysis/scan-incremental.ts --state .last-block    # custom state file
 *   npx tsx analysis/scan-incremental.ts --reset                # ignore saved state, rescan from 0
 *   npx tsx analysis/scan-incremental.ts --token 0x20C0...0001  # scan different token
 *
 * State file: plain text, single number = last fully processed block.
 * CSV: appended (not overwritten). Header written only on first run.
 */
import {
  getTip20TransferHistory,
  decodeMemo,
} from '../ts/src/index'
import { existsSync, readFileSync, writeFileSync, appendFileSync } from 'fs'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const DEFAULT_TOKEN: `0x${string}` = '0x20C0000000000000000000000000000000000000'

const BATCH_SIZE = 2000      // blocks per RPC batch
const FLUSH_EVERY = 100      // flush CSV + update state every N batches
const PROGRESS_EVERY = 50    // log progress every N batches

function parseArgs() {
  const args = process.argv.slice(2)
  let outFile = 'analysis/memos-all.csv'
  let stateFile = 'analysis/.last-block'
  let token: `0x${string}` = DEFAULT_TOKEN
  let reset = false

  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case '--out':
        outFile = args[++i]!
        break
      case '--state':
        stateFile = args[++i]!
        break
      case '--token':
        token = args[++i]! as `0x${string}`
        break
      case '--reset':
        reset = true
        break
      default:
        console.error(`Unknown arg: ${args[i]}`)
        process.exit(1)
    }
  }

  return { outFile, stateFile, token, reset }
}

async function getBlockNumber(): Promise<bigint> {
  const res = await fetch(RPC_URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_blockNumber', params: [] }),
  })
  const json = await res.json() as { result: string }
  return BigInt(json.result)
}

function readLastBlock(stateFile: string): bigint {
  if (!existsSync(stateFile)) return 0n
  const raw = readFileSync(stateFile, 'utf-8').trim()
  if (!raw) return 0n
  return BigInt(raw)
}

function saveLastBlock(stateFile: string, block: bigint): void {
  writeFileSync(stateFile, block.toString() + '\n')
}

function escCsv(s: string): string {
  if (s.includes(',') || s.includes('"') || s.includes('\n')) {
    return '"' + s.replace(/"/g, '""') + '"'
  }
  return s
}

const CSV_HEADER = [
  'block_number', 'tx_hash', 'log_index',
  'from', 'to', 'amount_raw', 'amount_human',
  'memo_raw', 'memo_format', 'memo_decoded',
  'memo_v1_type', 'memo_v1_ulid', 'memo_v1_issuer_tag',
].join(',')

async function main() {
  const { outFile, stateFile, token, reset } = parseArgs()

  const head = await getBlockNumber()
  const savedBlock = reset ? 0n : readLastBlock(stateFile)
  const fromBlock = savedBlock === 0n ? 0n : savedBlock + 1n
  const toBlock = head

  if (fromBlock > toBlock) {
    console.log(`Already up to date. Last scanned: ${savedBlock}, chain head: ${head}`)
    return
  }

  const rangeSize = Number(toBlock - fromBlock)
  const totalBatches = Math.ceil(rangeSize / BATCH_SIZE)

  console.log(`Chain head:    ${head}`)
  console.log(`Last scanned:  ${savedBlock === 0n ? '(none — first run)' : savedBlock}`)
  console.log(`Scanning:      ${fromBlock} .. ${toBlock}  (${rangeSize.toLocaleString()} blocks)`)
  console.log(`Batches:       ${totalBatches} × ${BATCH_SIZE} blocks`)
  console.log(`Output:        ${outFile}`)
  console.log(`State:         ${stateFile}\n`)

  const csvExists = existsSync(outFile) && readFileSync(outFile, 'utf-8').trim().length > 0
  if (!csvExists) {
    writeFileSync(outFile, CSV_HEADER + '\n')
  }

  let eventCount = 0
  let buffer: string[] = []
  let lastProcessedBlock = savedBlock

  for (let batch = 0; batch < totalBatches; batch++) {
    const batchFrom = fromBlock + BigInt(batch * BATCH_SIZE)
    const batchTo = batchFrom + BigInt(BATCH_SIZE) - 1n > toBlock
      ? toBlock
      : batchFrom + BigInt(BATCH_SIZE) - 1n

    try {
      const events = await getTip20TransferHistory({
        rpcUrl: RPC_URL,
        chainId: CHAIN_ID,
        token,
        fromBlock: batchFrom,
        toBlock: batchTo,
        batchSize: BATCH_SIZE,
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

        buffer.push([
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

      lastProcessedBlock = batchTo
    } catch (err) {
      console.error(`  batch ${batch + 1}/${totalBatches} ERROR:`, err)
      // Don't advance lastProcessedBlock — will retry this range next run
      break
    }

    if ((batch + 1) % FLUSH_EVERY === 0 || batch === totalBatches - 1) {
      if (buffer.length > 0) {
        appendFileSync(outFile, buffer.join('\n') + '\n')
        buffer = []
      }
      saveLastBlock(stateFile, lastProcessedBlock)
    }

    if ((batch + 1) % PROGRESS_EVERY === 0 || batch === totalBatches - 1) {
      const pct = ((batch + 1) / totalBatches * 100).toFixed(1)
      console.log(`  ${batch + 1}/${totalBatches} (${pct}%) — block ${lastProcessedBlock} — ${eventCount.toLocaleString()} events`)
    }
  }

  if (buffer.length > 0) {
    appendFileSync(outFile, buffer.join('\n') + '\n')
  }
  saveLastBlock(stateFile, lastProcessedBlock)

  console.log(`\nDone. ${eventCount.toLocaleString()} new events appended to ${outFile}`)
  console.log(`Last block saved: ${lastProcessedBlock}`)
}

main().catch(console.error)
