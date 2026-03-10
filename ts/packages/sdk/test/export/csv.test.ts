import { describe, it, expect } from "vitest";
import { exportCsv } from "../../src/export/csv";
import type { MatchResult } from "../../src/types";
import { encodeMemoV1 } from "../../src/memo/encode";
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
      from: "0x2222222222222222222222222222222222222222",
      amount: 10_000_000n,
      dueAt: 1709200000,
      meta: { invoiceId: "INV-001", customer: "Acme" },
    },
    ...overrides,
  };
}

describe("exportCsv", () => {
  it("produces header + data rows", () => {
    const csv = exportCsv([makeResult()]);
    const lines = csv.trim().split("\n");
    expect(lines.length).toBe(2);
  });

  it("includes all fixed columns in header", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    for (const col of [
      "timestamp",
      "block_number",
      "tx_hash",
      "log_index",
      "chain_id",
      "from",
      "to",
      "token",
      "amount_raw",
      "amount_human",
      "memo_raw",
      "memo_type",
      "memo_ulid",
      "memo_issuer_tag",
      "status",
      "expected_amount",
      "expected_from",
      "expected_to",
      "expected_due_at",
      "reason",
      "overpaid_by",
      "is_late",
      "remaining_amount",
    ]) {
      expect(header).toContain(col);
    }
  });

  it("includes meta_* columns", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    expect(header).toContain("meta_customer");
    expect(header).toContain("meta_invoiceId");
  });

  it("formats amount as human-readable", () => {
    const csv = exportCsv([makeResult()]);
    const dataRow = csv.split("\n")[1]!;
    expect(dataRow).toContain("10.000000");
  });

  it("includes log_index in data row", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const colIndex = header.split(",").indexOf("log_index");
    expect(dataRow.split(",")[colIndex]).toBe("0");
  });

  it("includes memo_issuer_tag in data row", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const colIndex = header.split(",").indexOf("memo_issuer_tag");
    expect(dataRow.split(",")[colIndex]).not.toBe("");
  });

  it("includes expected_from, expected_to, expected_due_at in data row", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const cols = header.split(",");
    const values = dataRow.split(",");
    expect(values[cols.indexOf("expected_from")]).toBe(
      "0x2222222222222222222222222222222222222222",
    );
    expect(values[cols.indexOf("expected_to")]).toBe("0x1111111111111111111111111111111111111111");
    expect(values[cols.indexOf("expected_due_at")]).toBe("1709200000");
  });

  it("includes overpaid_by and is_late in data row", () => {
    const csv = exportCsv([makeResult({ overpaidBy: 500n, isLate: true })]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const cols = header.split(",");
    const values = dataRow.split(",");
    expect(values[cols.indexOf("overpaid_by")]).toBe("500");
    expect(values[cols.indexOf("is_late")]).toBe("true");
  });

  it("handles empty results", () => {
    const csv = exportCsv([]);
    const lines = csv.trim().split("\n");
    expect(lines.length).toBe(1);
  });

  it("handles results without expected field", () => {
    const csv = exportCsv([makeResult({ expected: undefined, status: "no_memo" })]);
    const lines = csv.trim().split("\n");
    expect(lines.length).toBe(2);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const cols = header.split(",");
    const values = dataRow.split(",");
    expect(values[cols.indexOf("expected_amount")]).toBe("");
    expect(values[cols.indexOf("expected_from")]).toBe("");
  });

  it("escapes commas in values", () => {
    const result = makeResult();
    result.expected!.meta = { note: "hello, world" };
    const csv = exportCsv([result]);
    expect(csv).toContain('"hello, world"');
  });

  it("escapes double quotes in values", () => {
    const result = makeResult();
    result.expected!.meta = { note: 'said "hello"' };
    const csv = exportCsv([result]);
    expect(csv).toContain('"said ""hello"""');
  });

  it("escapes newlines in values", () => {
    const result = makeResult();
    result.expected!.meta = { note: "line1\nline2" };
    const csv = exportCsv([result]);
    expect(csv).toContain('"line1\nline2"');
  });

  it("escapes carriage returns in values", () => {
    const result = makeResult();
    result.expected!.meta = { note: "line1\rline2" };
    const csv = exportCsv([result]);
    expect(csv).toContain('"line1\rline2"');
  });

  it("includes status column", () => {
    const csv = exportCsv([makeResult({ status: "mismatch_amount" })]);
    expect(csv).toContain("mismatch_amount");
  });

  it("includes chain_id in data row", () => {
    const csv = exportCsv([makeResult()]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const colIndex = header.split(",").indexOf("chain_id");
    expect(colIndex).toBeGreaterThan(-1);
    expect(dataRow.split(",")[colIndex]).toBe("42431");
  });

  it("includes remaining_amount for partial results", () => {
    const csv = exportCsv([makeResult({ status: "partial", remainingAmount: 5_000_000n })]);
    const header = csv.split("\n")[0]!;
    const dataRow = csv.split("\n")[1]!;
    const colIndex = header.split(",").indexOf("remaining_amount");
    expect(colIndex).toBeGreaterThan(-1);
    expect(dataRow.split(",")[colIndex]).toBe("5000000");
  });
});

describe("formatAmount edge cases", () => {
  it("formats zero as 0.000000", () => {
    const result = exportCsv([
      { ...makeResult(), payment: { ...makeResult().payment, amount: 0n } },
    ]);
    const row = result.split("\n")[1]!;
    const cells = row.split(",");
    const amountHuman = cells[9]!; // amount_human column
    expect(amountHuman).toBe("0.000000");
  });

  it("formats sub-unit amount (1 unit = 0.000001)", () => {
    const result = exportCsv([
      { ...makeResult(), payment: { ...makeResult().payment, amount: 1n } },
    ]);
    const row = result.split("\n")[1]!;
    const cells = row.split(",");
    const amountHuman = cells[9]!;
    expect(amountHuman).toBe("0.000001");
  });
});
