#!/usr/bin/env npx tsx
/**
 * Scans a block range on Moderato testnet, collects all TransferWithMemo events,
 * decodes every memo, and prints a summary of what formats are used on-chain.
 *
 * Usage: npx tsx examples/13-memo-index.ts [fromBlock] [rangeSize]
 *   npx tsx examples/13-memo-index.ts              # last 500 blocks
 *   npx tsx examples/13-memo-index.ts 6504870 200  # specific range
 */
import { getTip20TransferHistory, decodeMemoV1, decodeMemoText, decodeMemo } from "../src/index";

const RPC_URL = "https://rpc.moderato.tempo.xyz";
const CHAIN_ID = 42431;
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";

type MemoEntry = {
  txHash: string;
  block: bigint;
  from: string;
  to: string;
  amount: bigint;
  memoRaw: string;
  format: "v1" | "text" | "binary";
  decoded: string;
};

async function getBlockNumber(): Promise<bigint> {
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "eth_blockNumber", params: [] }),
  });
  const json = (await res.json()) as { result: string };
  return BigInt(json.result);
}

async function main() {
  const head = await getBlockNumber();

  const fromBlock = process.argv[2] ? BigInt(process.argv[2]) : head - 500n;
  const rangeSize = process.argv[3] ? Number(process.argv[3]) : 500;
  const toBlock = fromBlock + BigInt(rangeSize);

  console.log(`Scanning blocks ${fromBlock} .. ${toBlock} (${rangeSize} blocks)`);
  console.log(`Chain head: ${head}\n`);

  const events = await getTip20TransferHistory({
    rpcUrl: RPC_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    fromBlock,
    toBlock,
    batchSize: 500,
  });

  console.log(`Found ${events.length} TransferWithMemo events\n`);

  if (events.length === 0) {
    console.log("No events in this range. Try a different block range.");
    return;
  }

  const index: MemoEntry[] = [];
  const formatCounts = { v1: 0, text: 0, binary: 0 };
  const textMemos = new Map<string, number>();
  const v1Types = new Map<string, number>();
  const v1Issuers = new Map<bigint, number>();
  const uniqueMemos = new Set<string>();

  for (const e of events) {
    if (!e.memoRaw) continue;

    uniqueMemos.add(e.memoRaw);
    const decoded = decodeMemo(e.memoRaw);

    let format: "v1" | "text" | "binary";
    let label: string;

    if (decoded && typeof decoded === "object") {
      format = "v1";
      label = `v1/${decoded.t}/${decoded.ulid.slice(0, 10)}...`;
      v1Types.set(decoded.t, (v1Types.get(decoded.t) ?? 0) + 1);
      v1Issuers.set(decoded.issuerTag, (v1Issuers.get(decoded.issuerTag) ?? 0) + 1);
    } else if (typeof decoded === "string") {
      format = "text";
      label = decoded;
      textMemos.set(decoded, (textMemos.get(decoded) ?? 0) + 1);
    } else {
      format = "binary";
      label = e.memoRaw.slice(0, 20) + "...";
    }

    formatCounts[format]++;
    index.push({
      txHash: e.txHash,
      block: e.blockNumber,
      from: e.from,
      to: e.to,
      amount: e.amount,
      memoRaw: e.memoRaw,
      format,
      decoded: label,
    });
  }

  console.log("=== Format Distribution ===\n");
  console.log(`  v1 structured: ${formatCounts.v1}`);
  console.log(`  text (UTF-8):  ${formatCounts.text}`);
  console.log(`  binary/other:  ${formatCounts.binary}`);
  console.log(`  unique memos:  ${uniqueMemos.size}`);

  if (v1Types.size > 0) {
    console.log("\n=== V1 Types ===\n");
    for (const [type, count] of [...v1Types.entries()].sort((a, b) => b[1] - a[1])) {
      console.log(`  ${type}: ${count}`);
    }
  }

  if (v1Issuers.size > 0) {
    console.log("\n=== V1 Issuer Tags ===\n");
    for (const [tag, count] of [...v1Issuers.entries()].sort((a, b) => b[1] - a[1])) {
      console.log(`  0x${tag.toString(16).padStart(16, "0")}: ${count} events`);
    }
  }

  if (textMemos.size > 0) {
    console.log("\n=== Text Memos (top 20) ===\n");
    const sorted = [...textMemos.entries()].sort((a, b) => b[1] - a[1]).slice(0, 20);
    for (const [text, count] of sorted) {
      const display = text.replace(/[\r\n\0]/g, "").trim();
      console.log(`  "${display}": ${count}`);
    }
  }

  console.log("\n=== Sample Events (first 15) ===\n");
  for (const entry of index.slice(0, 15)) {
    const amt = Number(entry.amount) / 1e6;
    console.log(
      `  [${entry.format.padEnd(6)}]`,
      `block=${entry.block}`,
      `${entry.from.slice(0, 8)}..→${entry.to.slice(0, 8)}..`,
      `${amt.toFixed(2)} pathUSD`,
      `memo="${entry.decoded}"`,
    );
  }

  if (index.length > 15) {
    console.log(`  ... and ${index.length - 15} more`);
  }

  const unknowns = index.filter((e) => e.format === "binary");
  if (unknowns.length > 0) {
    console.log("\n=== Unknown Binary Memos (hex dump) ===\n");
    const seen = new Set<string>();
    for (const entry of unknowns) {
      if (seen.has(entry.memoRaw)) continue;
      seen.add(entry.memoRaw);
      console.log(`  ${entry.memoRaw}`);
      const hex = entry.memoRaw.slice(2);
      let ascii = "  ";
      for (let i = 0; i < hex.length; i += 2) {
        const byte = parseInt(hex.slice(i, i + 2), 16);
        ascii += byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : ".";
      }
      console.log(ascii);
      if (seen.size >= 10) {
        console.log(`  ... and ${unknowns.length - 10} more`);
        break;
      }
    }
  }
}

main().catch(console.error);
