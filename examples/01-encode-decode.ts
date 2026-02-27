#!/usr/bin/env npx tsx
/**
 * Example: memo encode/decode — structured v1 and raw text formats.
 * Runs offline, no RPC needed.
 *
 * Usage: npx tsx examples/01-encode-decode.ts
 */
import {
  encodeMemoV1,
  decodeMemoV1,
  decodeMemoText,
  decodeMemo,
  issuerTagFromNamespace,
} from '../ts/src/index'

const tag = issuerTagFromNamespace('my-app')
console.log('issuerTag for "my-app":', tag, `(0x${tag.toString(16)})`)

const memo = encodeMemoV1({
  type: 'invoice',
  issuerTag: tag,
  ulid: '01MASW9NF6YW40J40H289H858P',
})
console.log('\nEncoded v1 memo:', memo)
console.log('Length:', memo.length, 'chars (0x + 64 hex = 32 bytes)')

const decoded = decodeMemoV1(memo)
if (decoded) {
  console.log('\nDecoded v1 memo:')
  console.log('  version:', decoded.v)
  console.log('  type:', decoded.t)
  console.log('  issuerTag:', decoded.issuerTag)
  console.log('  ulid:', decoded.ulid)
}

const textBytes = new TextEncoder().encode('PAY-595079')
const padded = new Uint8Array(32)
padded.set(textBytes)
let textHex = '0x'
for (const b of padded) textHex += b.toString(16).padStart(2, '0')

console.log('\nText memo hex:', textHex)
console.log('decodeMemoText:', decodeMemoText(textHex as `0x${string}`))

console.log('\nUnified decodeMemo:')
console.log('  v1 memo →', typeof decodeMemo(memo) === 'object' ? 'MemoV1 object' : decodeMemo(memo))
console.log('  text memo →', decodeMemo(textHex as `0x${string}`))
console.log('  all zeros →', decodeMemo('0x0000000000000000000000000000000000000000000000000000000000000000'))
