#!/usr/bin/env npx tsx
/**
 * Example: fetch both TransferWithMemo and plain Transfer events.
 * The includeTransferOnly flag picks up transfers that have no memo attached.
 *
 * Usage: npx tsx examples/06-transfer-only.ts
 */
import { getTip20TransferHistory, decodeMemo } from "../src/index";

const RPC_URL = "https://rpc.moderato.tempo.xyz";
const CHAIN_ID = 42431;
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";
const BLOCK = 6504870n;

async function main() {
  console.log("Fetching transfers (with and without memo) from block", BLOCK.toString(), "...\n");

  const allEvents = await getTip20TransferHistory({
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    fromBlock: BLOCK,
    toBlock: BLOCK + 100n,
    includeTransferOnly: true,
  });

  const memoEvents = await getTip20TransferHistory({
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    fromBlock: BLOCK,
    toBlock: BLOCK + 100n,
  });

  console.log(`Total events (with includeTransferOnly): ${allEvents.length}`);
  console.log(`Memo-only events:                        ${memoEvents.length}`);
  console.log(`Plain transfers (no memo):               ${allEvents.length - memoEvents.length}\n`);

  for (const e of allEvents.slice(0, 10)) {
    const amount = Number(e.amount) / 1e6;
    const tag = e.memoRaw ? "[MEMO]" : "[    ]";

    let memoStr = "";
    if (e.memoRaw) {
      const decoded = decodeMemo(e.memoRaw);
      memoStr =
        typeof decoded === "string"
          ? decoded
          : decoded && typeof decoded === "object"
            ? `v1/${decoded.t}/${decoded.ulid}`
            : e.memoRaw.slice(0, 16) + "...";
    }

    console.log(
      `  ${tag} block=${e.blockNumber}`,
      `${e.from.slice(0, 10)}...`,
      `-> ${e.to.slice(0, 10)}...`,
      `${amount.toFixed(2)} pathUSD`,
      memoStr ? `memo="${memoStr}"` : "",
    );
  }
}

main().catch(console.error);
