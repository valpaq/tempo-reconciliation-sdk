#!/usr/bin/env npx tsx
/**
 * End-to-end integration test on Tempo Moderato testnet.
 *
 * What this does:
 *   1. Encode a v1 memo (invoice type)
 *   2. Register the expected payment with the reconciler
 *   3. Start watching for incoming transfers
 *   4. Send transferWithMemo on-chain (sender → receiver)
 *   5. Watcher catches the event, reconciler matches it
 *   6. Print the result and exit
 *
 * Requirements:
 *   - SENDER_KEY env var: private key of the sending wallet (with pathUSD balance)
 *   - RECEIVER env var: recipient address (optional, defaults to sender)
 *
 * Get testnet tokens:
 *   cast rpc tempo_fundAddress 0xYourAddress --rpc-url https://rpc.moderato.tempo.xyz
 *
 * Usage:
 *   # Put SENDER_KEY (and optionally RECEIVER) in .env at the repo root, then:
 *   cd ts && pnpm e2e
 *
 *   # Or inline without .env:
 *   cd ts && SENDER_KEY=0x... pnpm e2e
 */
import {
  encodeMemoV1,
  issuerTagFromNamespace,
  decodeMemoV1,
  watchTip20Transfers,
  Reconciler,
  exportJson,
} from "../src/index";
import { createPublicClient, createWalletClient, http, parseAbi, formatUnits } from "viem";
import { privateKeyToAccount } from "viem/accounts";

// ── config ────────────────────────────────────────────────────────────────────

const RPC_URL = "https://rpc.moderato.tempo.xyz";
const CHAIN_ID = 42431;
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";
const DECIMALS = 6;
const AMOUNT = 1_000_000n; // 1 pathUSD
const TIMEOUT_MS = 30_000; // bail out if no event in 30s

const moderato = {
  id: CHAIN_ID,
  name: "Tempo Moderato",
  nativeCurrency: { name: "TEMPO", symbol: "TEMPO", decimals: 18 },
  rpcUrls: { default: { http: [RPC_URL] } },
} as const;

const tip20Abi = parseAbi([
  "function transferWithMemo(address to, uint256 amount, bytes32 memo)",
  "function balanceOf(address account) view returns (uint256)",
]);

// ── helpers ───────────────────────────────────────────────────────────────────

function log(msg: string) {
  console.log(msg);
}
function ok(msg: string) {
  console.log(`✓ ${msg}`);
}
function fail(msg: string) {
  console.error(`✗ ${msg}`);
  process.exit(1);
}

// ── main ──────────────────────────────────────────────────────────────────────

async function main() {
  const privateKey = process.env.SENDER_KEY as `0x${string}` | undefined;
  if (!privateKey) {
    fail("SENDER_KEY env var is required.\n  SENDER_KEY=0x... npx tsx examples/e2e-integration.ts");
    return;
  }

  const sender = privateKeyToAccount(privateKey);
  const receiver = (process.env.RECEIVER as `0x${string}` | undefined) ?? sender.address;

  const transport = http(RPC_URL);
  const publicClient = createPublicClient({ chain: moderato, transport });
  const walletClient = createWalletClient({ account: sender, chain: moderato, transport });

  log(`\n${"─".repeat(56)}`);
  log("  tempo-reconcile E2E integration test");
  log(`${"─".repeat(56)}`);
  log(`  Sender:   ${sender.address}`);
  log(`  Receiver: ${receiver}`);
  log(`  Token:    pathUSD (${AMOUNT / BigInt(10 ** DECIMALS)} USDC)`);
  log(`  RPC:      ${RPC_URL}`);
  log(`${"─".repeat(56)}\n`);

  // ── step 1: check balance ──────────────────────────────────────────────────
  log("Step 1: Check sender balance");
  const balance = await publicClient.readContract({
    address: PATH_USD,
    abi: tip20Abi,
    functionName: "balanceOf",
    args: [sender.address],
  });
  log(`  pathUSD: ${formatUnits(balance, DECIMALS)}`);

  if (balance < AMOUNT) {
    fail(
      `Insufficient pathUSD balance (${formatUnits(balance, DECIMALS)}).\n` +
        `  Fund with: cast rpc tempo_fundAddress ${sender.address} --rpc-url ${RPC_URL}`,
    );
  }
  ok("Balance sufficient\n");

  // ── step 2: encode memo ────────────────────────────────────────────────────
  log("Step 2: Encode invoice memo");
  const ISSUER = issuerTagFromNamespace("e2e-integration");
  const ulid = generateUlid();
  const memoRaw = encodeMemoV1({ type: "invoice", issuerTag: ISSUER, ulid });
  const decoded = decodeMemoV1(memoRaw)!;

  log(`  ULID:     ${ulid}`);
  log(`  memo:     ${memoRaw}`);
  log(`  issuerTag: 0x${decoded.issuerTag.toString(16)}`);
  ok("Memo encoded\n");

  // ── step 3: register expected payment ─────────────────────────────────────
  log("Step 3: Register expected payment");
  const reconciler = new Reconciler({ issuerTag: ISSUER });
  reconciler.expect({
    memoRaw,
    token: PATH_USD,
    to: receiver,
    amount: AMOUNT,
    meta: { ulid, env: "moderato-testnet", script: "e2e-integration" },
  });
  ok("Expected payment registered\n");

  // ── step 4: start watcher (before sending, to avoid missing the event) ────
  log("Step 4: Start watcher");
  const currentBlock = await publicClient.getBlockNumber();
  log(`  Watching from block ${currentBlock}`);

  let resolveMatch: (txHash: string) => void;
  const matchPromise = new Promise<string>((r) => {
    resolveMatch = r;
  });

  const stop = watchTip20Transfers(
    {
      rpcUrl: RPC_URL,
      chainId: CHAIN_ID,
      token: PATH_USD,
      to: receiver,
      startBlock: currentBlock,
      pollIntervalMs: 500,
      onError: (err) => log(`  watcher error: ${err.message}`),
    },
    (event) => {
      log(`\n  Incoming event: tx=${event.txHash.slice(0, 18)}... block=${event.blockNumber}`);
      const result = reconciler.ingest(event);
      log(`  Reconciler:     status=${result.status}`);
      resolveMatch(event.txHash);
    },
  );
  ok("Watcher started\n");

  // ── step 5: send transaction ───────────────────────────────────────────────
  log("Step 5: Send transferWithMemo on-chain");
  log(`  Sending ${formatUnits(AMOUNT, DECIMALS)} pathUSD → ${receiver.slice(0, 10)}...`);

  const txHash = await walletClient.writeContract({
    address: PATH_USD,
    abi: tip20Abi,
    functionName: "transferWithMemo",
    args: [receiver, AMOUNT, memoRaw],
  });
  log(`  tx hash: ${txHash}`);

  const receipt = await publicClient.waitForTransactionReceipt({ hash: txHash });
  log(`  status:  ${receipt.status}`);
  log(`  block:   ${receipt.blockNumber}`);
  ok("Transaction confirmed on-chain\n");

  // ── step 6: wait for reconciler match ─────────────────────────────────────
  log("Step 6: Waiting for watcher to catch the event...");
  const matched = await Promise.race([
    matchPromise,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error(`Timeout after ${TIMEOUT_MS}ms`)), TIMEOUT_MS),
    ),
  ]).catch((err: Error) => {
    stop();
    fail(err.message);
  });

  stop();

  if (!matched) return;

  // ── step 7: print report ───────────────────────────────────────────────────
  log("\nStep 7: Reconciliation report");
  const report = reconciler.report();
  const summary = report.summary;

  log(`  totalExpected: ${summary.totalExpected}`);
  log(`  matched:       ${summary.matchedCount}`);
  log(`  pending:       ${summary.pendingCount}`);
  log(`  issues:        ${summary.issueCount}`);

  if (summary.matchedCount !== 1) {
    fail(`Expected 1 matched payment, got ${summary.matchedCount}`);
  }

  const matchedResult = report.matched[0]!;
  log(`\n  ✓ Invoice matched!`);
  log(`    ulid:      ${matchedResult.expected?.meta?.ulid}`);
  log(`    amount:    ${formatUnits(matchedResult.payment.amount, DECIMALS)} pathUSD`);
  log(`    txHash:    ${matchedResult.payment.txHash}`);
  log(`    block:     ${matchedResult.payment.blockNumber}`);
  log(`    logIndex:  ${matchedResult.payment.logIndex}`);

  log("\n  Full result (JSON):");
  log("  " + exportJson([matchedResult]).split("\n").join("\n  "));

  log(`\n${"─".repeat(56)}`);
  log("  E2E PASS — payment sent, caught, and reconciled.");
  log(`${"─".repeat(56)}\n`);

  // Force exit: watcher stop() cancels the next poll but an in-flight
  // eth_getLogs request keeps the event loop alive. For a script, just exit.
  process.exit(0);
}

// ── tiny ULID generator (no external deps) ────────────────────────────────────
// Uses crypto.randomBytes for the random part and current time for the timestamp.
function generateUlid(): string {
  const CHARS = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
  const now = Date.now();
  let ulid = "";

  // 10 chars timestamp (48 bits)
  let t = now;
  for (let i = 9; i >= 0; i--) {
    ulid = CHARS[t % 32]! + ulid;
    t = Math.floor(t / 32);
  }

  // 16 chars random (80 bits)
  const bytes = new Uint8Array(10);
  crypto.getRandomValues(bytes);
  let rand = 0n;
  for (const b of bytes) rand = (rand << 8n) | BigInt(b);
  for (let i = 0; i < 16; i++) {
    ulid += CHARS[Number(rand % 32n)];
    rand >>= 5n;
  }

  return ulid;
}

main().catch(console.error);
