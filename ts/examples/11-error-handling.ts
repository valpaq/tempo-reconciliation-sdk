#!/usr/bin/env npx tsx
/**
 * Example: error handling — how the SDK behaves with invalid or unexpected input.
 * Demonstrates every MatchStatus and decoder edge case.
 *
 * Usage: npx tsx examples/11-error-handling.ts
 */
import {
  decodeMemoV1,
  decodeMemoText,
  decodeMemo,
  encodeMemoV1,
  issuerTagFromNamespace,
  Reconciler,
} from "../src/index";
import type { PaymentEvent } from "../src/index";

const TOKEN: `0x${string}` = "0x20C0000000000000000000000000000000000000";
const ADDR_A: `0x${string}` = "0x1111111111111111111111111111111111111111";
const ADDR_B: `0x${string}` = "0x2222222222222222222222222222222222222222";
const ADDR_C: `0x${string}` = "0x3333333333333333333333333333333333333333";

function makeEvent(overrides: Partial<PaymentEvent> = {}): PaymentEvent {
  return {
    chainId: 42431,
    blockNumber: 100n,
    txHash: `0x${"aa".repeat(32)}` as `0x${string}`,
    logIndex: 0,
    token: TOKEN,
    from: ADDR_A,
    to: ADDR_B,
    amount: 50_000_000n,
    ...overrides,
  };
}

console.log("=== Decoder Edge Cases ===\n");

const allZeros =
  "0x0000000000000000000000000000000000000000000000000000000000000000" as `0x${string}`;
const garbage =
  "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`;
const tooShort = "0x1234" as `0x${string}`;

console.log(`  decodeMemoV1(all zeros):  ${decodeMemoV1(allZeros)}`);
console.log(`  decodeMemoV1(garbage):    ${decodeMemoV1(garbage)}`);
console.log(`  decodeMemoV1(too short):  ${decodeMemoV1(tooShort)}`);
console.log(`  decodeMemoText(all zeros): ${decodeMemoText(allZeros)}`);
console.log(`  decodeMemoText(garbage):  ${decodeMemoText(garbage)}`);
console.log(`  decodeMemo(all zeros):    ${decodeMemo(allZeros)}`);
console.log(`  decodeMemo(garbage):      ${decodeMemo(garbage)}`);

console.log("\n=== Every MatchStatus ===\n");

const tag = issuerTagFromNamespace("test-app");
const memo = encodeMemoV1({ type: "invoice", issuerTag: tag, ulid: "01MASW9NF6YW40J40H289H858P" });
const otherMemo = encodeMemoV1({
  type: "invoice",
  issuerTag: tag,
  ulid: "01MASW9NF6YW40J40H289H999Z",
});

const r1 = new Reconciler();
const noMemoResult = r1.ingest(makeEvent());
console.log(`  no_memo:          ${noMemoResult.status}`);

const r2 = new Reconciler();
const unknownResult = r2.ingest(makeEvent({ memoRaw: memo }));
console.log(`  unknown_memo:     ${unknownResult.status}`);

const r3 = new Reconciler();
r3.expect({ memoRaw: memo, token: TOKEN, to: ADDR_B, amount: 50_000_000n });
const matchedResult = r3.ingest(makeEvent({ memoRaw: memo }));
console.log(`  matched:          ${matchedResult.status}`);

const r4 = new Reconciler();
r4.expect({ memoRaw: memo, token: TOKEN, to: ADDR_B, amount: 100_000_000n });
const amountResult = r4.ingest(makeEvent({ memoRaw: memo }));
console.log(`  mismatch_amount:  ${amountResult.status} (${amountResult.reason})`);

const r5 = new Reconciler();
r5.expect({ memoRaw: memo, token: TOKEN, to: ADDR_C, amount: 50_000_000n });
const partyResult = r5.ingest(makeEvent({ memoRaw: memo }));
console.log(`  mismatch_party:   ${partyResult.status} (${partyResult.reason})`);

const r6 = new Reconciler({ rejectExpired: true });
r6.expect({ memoRaw: memo, token: TOKEN, to: ADDR_B, amount: 50_000_000n, dueAt: 1000 });
const expiredResult = r6.ingest(makeEvent({ memoRaw: memo, timestamp: 2000 }));
console.log(`  expired:          ${expiredResult.status} (${expiredResult.reason})`);

const r7 = new Reconciler({ allowPartial: true });
r7.expect({ memoRaw: memo, token: TOKEN, to: ADDR_B, amount: 100_000_000n });
const partialResult = r7.ingest(makeEvent({ memoRaw: memo, amount: 30_000_000n }));
console.log(
  `  partial:          ${partialResult.status} (remaining: ${Number(partialResult.remainingAmount) / 1e6})`,
);

console.log("\n=== Duplicate expect() ===\n");
const r8 = new Reconciler();
r8.expect({ memoRaw: otherMemo, token: TOKEN, to: ADDR_B, amount: 50_000_000n });
try {
  r8.expect({ memoRaw: otherMemo, token: TOKEN, to: ADDR_B, amount: 50_000_000n });
  console.log("  (no error thrown)");
} catch (err) {
  console.log(`  Error: ${(err as Error).message}`);
}
