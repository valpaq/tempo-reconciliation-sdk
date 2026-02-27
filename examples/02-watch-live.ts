#!/usr/bin/env npx tsx
/**
 * Example: watch pathUSD TransferWithMemo events on Moderato testnet in real-time.
 * Press Ctrl+C to stop.
 *
 * Usage: npx tsx examples/02-watch-live.ts
 */
import { watchTip20Transfers, decodeMemo } from '../ts/src/index'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'

console.log('Watching pathUSD transfers on Moderato testnet...')
console.log(`RPC: ${RPC_URL} | Chain: ${CHAIN_ID} | Token: ${PATH_USD}`)
console.log('Press Ctrl+C to stop.\n')

const unsubscribe = watchTip20Transfers(
  {
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    pollIntervalMs: 2000,
    onError: (err) => console.error('Poll error:', err.message),
  },
  (event) => {
    const memoDisplay = event.memoRaw
      ? decodeMemo(event.memoRaw) ?? event.memoRaw.slice(0, 20) + '...'
      : '(none)'

    const amount = Number(event.amount) / 1e6

    console.log(
      `[block ${event.blockNumber}]`,
      `${event.from.slice(0, 10)}...`,
      `→ ${event.to.slice(0, 10)}...`,
      `${amount.toFixed(2)} pathUSD`,
      `memo: ${typeof memoDisplay === 'string' ? memoDisplay : `v1/${memoDisplay.t}/${memoDisplay.ulid}`}`,
    )
  },
)

process.on('SIGINT', () => {
  console.log('\nStopping watcher...')
  unsubscribe()
  process.exit(0)
})
