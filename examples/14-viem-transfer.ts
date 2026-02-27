#!/usr/bin/env npx tsx
/**
 * Example: send a TIP-20 payment with memo using viem.
 *
 * Shows the full flow:
 *   1. Encode a structured v1 memo
 *   2. Send transferWithMemo via viem walletClient
 *   3. Wait for receipt
 *   4. Decode the memo back from the receipt logs
 *
 * Requirements:
 *   - A funded account on Moderato testnet
 *   - Set PRIVATE_KEY env var (or run without for dry-run)
 *
 * Usage (run from ts/ to resolve viem):
 *   cd ts && NODE_PATH=node_modules npx tsx ../examples/14-viem-transfer.ts
 *   cd ts && NODE_PATH=node_modules PRIVATE_KEY=0x... npx tsx ../examples/14-viem-transfer.ts
 */
import {
  encodeMemoV1,
  decodeMemo,
  issuerTagFromNamespace,
} from '../ts/src/index'
import {
  createPublicClient,
  createWalletClient,
  http,
  parseAbi,
  formatUnits,
} from 'viem'
import { privateKeyToAccount } from 'viem/accounts'

const RPC_URL = 'https://rpc.moderato.tempo.xyz'
const CHAIN_ID = 42431
const PATH_USD: `0x${string}` = '0x20C0000000000000000000000000000000000000'
const DECIMALS = 6

const moderato = {
  id: CHAIN_ID,
  name: 'Tempo Moderato',
  nativeCurrency: { name: 'TEMPO', symbol: 'TEMPO', decimals: 18 },
  rpcUrls: { default: { http: [RPC_URL] } },
} as const

const tip20Abi = parseAbi([
  'function transferWithMemo(address to, uint256 amount, bytes32 memo)',
  'function balanceOf(address account) view returns (uint256)',
  'event TransferWithMemo(address indexed from, address indexed to, uint256 amount, bytes32 indexed memo)',
])

async function main() {
  const privateKey = process.env.PRIVATE_KEY as `0x${string}` | undefined
  if (!privateKey) {
    console.log('No PRIVATE_KEY env var set. Running in dry-run mode.\n')
    dryRun()
    return
  }

  const account = privateKeyToAccount(privateKey)
  const transport = http(RPC_URL)

  const publicClient = createPublicClient({ chain: moderato, transport })
  const walletClient = createWalletClient({ account, chain: moderato, transport })

  console.log('Sender:', account.address)

  const balance = await publicClient.readContract({
    address: PATH_USD,
    abi: tip20Abi,
    functionName: 'balanceOf',
    args: [account.address],
  })
  console.log('pathUSD balance:', formatUnits(balance, DECIMALS))

  if (balance === 0n) {
    console.log('No pathUSD balance. Get testnet tokens from the faucet.')
    return
  }

  const ISSUER = issuerTagFromNamespace('viem-example')
  const memo = encodeMemoV1({
    type: 'invoice',
    issuerTag: ISSUER,
    ulid: '01JNRX0KD42T3H9XJGCH5BKRWM',
  })

  console.log('\nMemo (bytes32):', memo)
  console.log('Decoded:', decodeMemo(memo))

  const recipient = account.address

  const amount = 10_000n // 0.01 pathUSD

  console.log(`\nSending ${formatUnits(amount, DECIMALS)} pathUSD to ${recipient.slice(0, 10)}...`)

  const hash = await walletClient.writeContract({
    address: PATH_USD,
    abi: tip20Abi,
    functionName: 'transferWithMemo',
    args: [recipient, amount, memo],
  })

  console.log('tx hash:', hash)

  const receipt = await publicClient.waitForTransactionReceipt({ hash })
  console.log('status:', receipt.status)
  console.log('block:', receipt.blockNumber)
  console.log('gas used:', receipt.gasUsed)

  console.log('\nTransaction logs:')
  for (const log of receipt.logs) {
    if (log.topics[3]) {
      const memoFromLog = log.topics[3] as `0x${string}`
      const decoded = decodeMemo(memoFromLog)
      console.log('  memo from log:', memoFromLog.slice(0, 20) + '...')
      if (decoded && typeof decoded === 'object') {
        console.log('  decoded: v1', decoded.t, 'ulid=' + decoded.ulid)
      }
    }
  }
}

/**
 * Dry-run: shows what WOULD happen without sending a real transaction.
 */
function dryRun() {
  const ISSUER = issuerTagFromNamespace('viem-example')

  const memo = encodeMemoV1({
    type: 'invoice',
    issuerTag: ISSUER,
    ulid: '01JNRX0KD42T3H9XJGCH5BKRWM',
  })

  console.log('=== Dry Run (no private key) ===\n')
  console.log('Memo encoded:', memo)
  console.log('Decoded:', decodeMemo(memo))

  console.log('\nTo send this on-chain with viem:\n')
  console.log('  await walletClient.writeContract({')
  console.log(`    address: '${PATH_USD}',`)
  console.log('    abi: tip20Abi,')
  console.log("    functionName: 'transferWithMemo',")
  console.log(`    args: ['0xRecipient', 10_000_000n, '${memo}'],`)
  console.log('  })')
  console.log('\nSet PRIVATE_KEY env var to run for real.')
}

main().catch(console.error)
