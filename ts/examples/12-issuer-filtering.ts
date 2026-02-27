#!/usr/bin/env npx tsx
/**
 * Example: reconciler with issuerTag filtering.
 * Two apps share the same token and recipient address, but their memos
 * have different issuerTags. The reconciler only matches its own memos.
 *
 * Usage: npx tsx examples/12-issuer-filtering.ts
 */
import { Reconciler, encodeMemoV1, issuerTagFromNamespace, decodeMemoV1 } from "../src/index";
import type { PaymentEvent } from "../src/index";

const TOKEN: `0x${string}` = "0x20C0000000000000000000000000000000000000";
const RECIPIENT: `0x${string}` = "0x2222222222222222222222222222222222222222";
const SENDER: `0x${string}` = "0x1111111111111111111111111111111111111111";

const tagA = issuerTagFromNamespace("app-alpha");
const tagB = issuerTagFromNamespace("app-beta");

console.log("=== Issuer Tag Filtering ===\n");
console.log(`app-alpha issuerTag: ${tagA} (0x${tagA.toString(16).padStart(16, "0")})`);
console.log(`app-beta  issuerTag: ${tagB} (0x${tagB.toString(16).padStart(16, "0")})`);

const memoA = encodeMemoV1({
  type: "invoice",
  issuerTag: tagA,
  ulid: "01MASW9NF6YW40J40H289H858P",
});
const memoB = encodeMemoV1({
  type: "invoice",
  issuerTag: tagB,
  ulid: "01MASW9NF6YW40J40H289H999Z",
});

console.log(`\nmemo from app-alpha: ${memoA.slice(0, 30)}...`);
console.log(`memo from app-beta:  ${memoB.slice(0, 30)}...`);

const decodedA = decodeMemoV1(memoA)!;
const decodedB = decodeMemoV1(memoB)!;
console.log(`\nDecoded app-alpha: issuerTag=${decodedA.issuerTag}, type=${decodedA.t}`);
console.log(`Decoded app-beta:  issuerTag=${decodedB.issuerTag}, type=${decodedB.t}`);

console.log("\n--- Reconciler (app-alpha only) ---\n");
const reconciler = new Reconciler({ issuerTag: tagA });

reconciler.expect({ memoRaw: memoA, token: TOKEN, to: RECIPIENT, amount: 50_000_000n });
reconciler.expect({ memoRaw: memoB, token: TOKEN, to: RECIPIENT, amount: 25_000_000n });

const events: PaymentEvent[] = [
  {
    chainId: 42431,
    blockNumber: 100n,
    txHash: `0x${"aa".repeat(32)}` as `0x${string}`,
    logIndex: 0,
    token: TOKEN,
    from: SENDER,
    to: RECIPIENT,
    amount: 50_000_000n,
    memoRaw: memoA,
  },
  {
    chainId: 42431,
    blockNumber: 101n,
    txHash: `0x${"bb".repeat(32)}` as `0x${string}`,
    logIndex: 0,
    token: TOKEN,
    from: SENDER,
    to: RECIPIENT,
    amount: 25_000_000n,
    memoRaw: memoB,
  },
];

for (const event of events) {
  const result = reconciler.ingest(event);
  const decoded = decodeMemoV1(event.memoRaw!)!;
  const app = decoded.issuerTag === tagA ? "app-alpha" : "app-beta";
  console.log(
    `  ${app} event -> status: ${result.status}${result.reason ? ` (${result.reason})` : ""}`,
  );
}

const report = reconciler.report();
console.log(`\nMatched: ${report.summary.matchedCount}`);
console.log(`Unknown: ${report.summary.unknownMemoCount} (filtered by issuerTag)`);
console.log(`Pending: ${report.summary.pendingCount}`);
