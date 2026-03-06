#!/usr/bin/env npx tsx
/**
 * Example: memo encoding and decoding — all formats and edge cases.
 * Runs offline, no RPC needed.
 *
 * Usage: npx tsx examples/09-memo-formats.ts
 */
import {
  encodeMemoV1,
  decodeMemoV1,
  decodeMemoText,
  decodeMemo,
  issuerTagFromNamespace,
  ulidToBytes16,
  bytes16ToUlid,
} from "../src/index";
import type { MemoType } from "../src/index";

console.log("=== Issuer Tags ===\n");
const namespaces = ["my-app", "payroll-app", "billing-service", "marketplace"];
for (const ns of namespaces) {
  const tag = issuerTagFromNamespace(ns);
  console.log(`  "${ns}" -> ${tag} (0x${tag.toString(16).padStart(16, "0")})`);
}

console.log("\n=== All Memo Types ===\n");
const types: MemoType[] = ["invoice", "payroll", "refund", "batch", "subscription", "custom"];
const tag = issuerTagFromNamespace("demo-app");
const ulid = "01MASW9NF6YW40J40H289H858P";

for (const t of types) {
  const encoded = encodeMemoV1({ type: t, issuerTag: tag, ulid });
  const decoded = decodeMemoV1(encoded)!;
  console.log(`  ${t.padEnd(14)} -> ${encoded.slice(0, 30)}...`);
  console.log(`${"".padEnd(18)} <- v=${decoded.v} t=${decoded.t} ulid=${decoded.ulid}`);
}

console.log("\n=== Text Memos (ecosystem format) ===\n");
const textMemos = ["PAY-595079", "dropsnap", "tx1352", "invoice-2024-001"];
for (const text of textMemos) {
  const bytes = new TextEncoder().encode(text);
  const padded = new Uint8Array(32);
  padded.set(bytes);
  let hex = "0x" as string;
  for (const b of padded) hex += b.toString(16).padStart(2, "0");
  const hexTyped = hex as `0x${string}`;

  const decoded = decodeMemoText(hexTyped);
  console.log(`  "${text}" -> ${hexTyped.slice(0, 30)}...`);
  console.log(`  decodeMemoText: "${decoded}"`);
}

console.log("\n=== Unified Decoder ===\n");
const v1Memo = encodeMemoV1({ type: "invoice", issuerTag: tag, ulid });

const textBytes = new TextEncoder().encode("PAY-123");
const textPadded = new Uint8Array(32);
textPadded.set(textBytes);
let textHex = "0x" as string;
for (const b of textPadded) textHex += b.toString(16).padStart(2, "0");

const allZeros =
  "0x0000000000000000000000000000000000000000000000000000000000000000" as `0x${string}`;

console.log(
  `  v1 memo:   ${typeof decodeMemo(v1Memo) === "object" ? "MemoV1 object" : decodeMemo(v1Memo)}`,
);
console.log(`  text memo: ${decodeMemo(textHex as `0x${string}`)}`);
console.log(`  all zeros: ${decodeMemo(allZeros)}`);

console.log("\n=== ULID Round-trip ===\n");
const testUlid = "01MASW9NF6YW40J40H289H858P";
const bytes16 = ulidToBytes16(testUlid);
const recovered = bytes16ToUlid(bytes16);
console.log(`  Original:  ${testUlid}`);
console.log(
  `  Bytes:     [${Array.from(bytes16)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join(" ")}]`,
);
console.log(`  Recovered: ${recovered}`);
console.log(`  Match:     ${testUlid === recovered}`);
