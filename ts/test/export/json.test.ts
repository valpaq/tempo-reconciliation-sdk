import { describe, it, expect } from "vitest";
import { exportJson, exportJsonl } from "../../src/export/json";
import type { MatchResult } from "../../src/types";
import { encodeMemoV1 } from "../../src/memo/encode";
import { decodeMemoV1 } from "../../src/memo/decode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";

const TAG = issuerTagFromNamespace("test-app");
const MEMO = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: "01MASW9NF6YW40J40H289H858P" });

function makeResult(overrides: Partial<MatchResult> = {}): MatchResult {
  return {
    status: "matched",
    payment: {
      chainId: 42431,
      blockNumber: 100n,
      txHash: "0xaaaa000000000000000000000000000000000000000000000000000000000001",
      logIndex: 0,
      token: "0x20C0000000000000000000000000000000000000",
      from: "0x2222222222222222222222222222222222222222",
      to: "0x1111111111111111111111111111111111111111",
      amount: 10_000_000n,
      memoRaw: MEMO,
      timestamp: 1709123456,
    },
    expected: {
      memoRaw: MEMO,
      token: "0x20C0000000000000000000000000000000000000",
      to: "0x1111111111111111111111111111111111111111",
      amount: 10_000_000n,
      meta: { invoiceId: "INV-001" },
    },
    ...overrides,
  };
}

describe("exportJson", () => {
  it("returns valid JSON", () => {
    const json = exportJson([makeResult()]);
    const parsed = JSON.parse(json);
    expect(Array.isArray(parsed)).toBe(true);
    expect(parsed).toHaveLength(1);
  });

  it("serializes bigint as string", () => {
    const json = exportJson([makeResult()]);
    const parsed = JSON.parse(json);
    expect(parsed[0].payment.amount).toBe("10000000");
    expect(parsed[0].payment.blockNumber).toBe("100");
  });

  it("handles empty array", () => {
    const json = exportJson([]);
    expect(JSON.parse(json)).toEqual([]);
  });

  it("pretty-prints with 2-space indent", () => {
    const json = exportJson([makeResult()]);
    expect(json).toContain("\n");
    expect(json).toContain("  ");
  });

  it("preserves status field", () => {
    const json = exportJson([makeResult({ status: "mismatch_amount" })]);
    const parsed = JSON.parse(json);
    expect(parsed[0].status).toBe("mismatch_amount");
  });

  it("serializes Uint8Array fields as number arrays, not Buffer-like objects", () => {
    const decoded = decodeMemoV1(MEMO);
    const resultWithMemo = makeResult({
      payment: {
        chainId: 42431,
        blockNumber: 100n,
        txHash: "0xaaaa000000000000000000000000000000000000000000000000000000000001",
        logIndex: 0,
        token: "0x20C0000000000000000000000000000000000000",
        from: "0x2222222222222222222222222222222222222222",
        to: "0x1111111111111111111111111111111111111111",
        amount: 10_000_000n,
        memoRaw: MEMO,
        memo: decoded,
      },
    });

    const json = exportJson([resultWithMemo]);
    const parsed = JSON.parse(json);
    const memo = parsed[0].payment.memo;

    // Uint8Array fields must be plain number arrays, not {"0":1,"1":2,...} objects
    expect(Array.isArray(memo.salt)).toBe(true);
    expect(Array.isArray(memo.id16)).toBe(true);
    // Confirm every element is a plain number (not an object key like "0", "1")
    for (const byte of memo.salt as unknown[]) {
      expect(typeof byte).toBe("number");
    }
  });
});

describe("exportJsonl", () => {
  it("produces one line per result", () => {
    const results = [makeResult(), makeResult({ status: "no_memo" })];
    const jsonl = exportJsonl(results);
    const lines = jsonl.trim().split("\n");
    expect(lines).toHaveLength(2);
  });

  it("each line is valid JSON", () => {
    const results = [makeResult(), makeResult({ status: "no_memo" })];
    const jsonl = exportJsonl(results);
    const lines = jsonl.trim().split("\n");
    for (const line of lines) {
      expect(() => JSON.parse(line)).not.toThrow();
    }
  });

  it("ends with newline", () => {
    const jsonl = exportJsonl([makeResult()]);
    expect(jsonl.endsWith("\n")).toBe(true);
  });

  it("serializes bigint as string", () => {
    const jsonl = exportJsonl([makeResult()]);
    const parsed = JSON.parse(jsonl.trim());
    expect(parsed.payment.amount).toBe("10000000");
  });

  it("handles empty array", () => {
    const jsonl = exportJsonl([]);
    expect(jsonl).toBe("\n");
  });
});
