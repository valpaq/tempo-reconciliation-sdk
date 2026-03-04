import { describe, it, expect } from "vitest";
import { Reconciler } from "../../src/reconciler/reconciler";
import { InMemoryStore } from "../../src/reconciler/store";
import { encodeMemoV1 } from "../../src/memo/encode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";
import type { PaymentEvent, ExpectedPayment } from "../../src/types";

const TAG = issuerTagFromNamespace("test-app");
const TOKEN = "0x20C0000000000000000000000000000000000000" as const;
const ADDR = "0x1111111111111111111111111111111111111111" as const;
const SENDER = "0x2222222222222222222222222222222222222222" as const;

function makeMemo(ulid = "01MASW9NF6YW40J40H289H858P") {
  return encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid });
}

function makeEvent(overrides: Partial<PaymentEvent> = {}): PaymentEvent {
  return {
    chainId: 42431,
    blockNumber: 100n,
    txHash: "0xaaaa000000000000000000000000000000000000000000000000000000000001",
    logIndex: 0,
    token: TOKEN,
    from: SENDER,
    to: ADDR,
    amount: 10_000_000n,
    memoRaw: makeMemo(),
    ...overrides,
  };
}

function makeExpected(overrides: Partial<ExpectedPayment> = {}): ExpectedPayment {
  return {
    memoRaw: makeMemo(),
    token: TOKEN,
    to: ADDR,
    amount: 10_000_000n,
    ...overrides,
  };
}

describe("Reconciler", () => {
  it("matches a correct payment", () => {
    const r = new Reconciler();
    r.expect(makeExpected());
    const result = r.ingest(makeEvent());
    expect(result.status).toBe("matched");
    expect(result.expected).toBeDefined();
  });

  it("returns no_memo for events without memo", () => {
    const r = new Reconciler();
    r.expect(makeExpected());
    const result = r.ingest(makeEvent({ memoRaw: undefined }));
    expect(result.status).toBe("no_memo");
  });

  it("returns unknown_memo for unregistered memo", () => {
    const r = new Reconciler();
    const result = r.ingest(makeEvent());
    expect(result.status).toBe("unknown_memo");
  });

  it("returns cached result on second ingest of same event", () => {
    const r = new Reconciler();
    r.expect(makeExpected());
    const event = makeEvent();
    r.ingest(event);
    const result = r.ingest(event);
    expect(result.status).toBe("matched");
  });

  it("returns mismatch_amount for underpayment", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 5_000_000n }));
    expect(result.status).toBe("mismatch_amount");
    expect(result.reason).toContain("underpaid");
  });

  it("matches overpayment by default", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 15_000_000n }));
    expect(result.status).toBe("matched");
    expect(result.overpaidBy).toBe(5_000_000n);
  });

  it("rejects overpayment when allowOverpayment=false", () => {
    const r = new Reconciler({ allowOverpayment: false });
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 15_000_000n }));
    expect(result.status).toBe("mismatch_amount");
    expect(result.overpaidBy).toBe(5_000_000n);
  });

  it("returns mismatch_party for wrong recipient", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ to: ADDR }));
    const result = r.ingest(makeEvent({ to: "0x9999999999999999999999999999999999999999" }));
    expect(result.status).toBe("mismatch_party");
    expect(result.reason).toContain("recipient");
  });

  it("returns mismatch_token for wrong token", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ token: TOKEN }));
    const result = r.ingest(makeEvent({ token: "0x20C0000000000000000000000000000000000001" }));
    expect(result.status).toBe("mismatch_token");
    expect(result.reason).toContain("token");
  });

  it("ignores sender by default", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ from: SENDER }));
    const result = r.ingest(makeEvent({ from: "0x9999999999999999999999999999999999999999" }));
    expect(result.status).toBe("matched");
  });

  it("checks sender when strictSender=true", () => {
    const r = new Reconciler({ strictSender: true });
    r.expect(makeExpected({ from: SENDER }));
    const result = r.ingest(makeEvent({ from: "0x9999999999999999999999999999999999999999" }));
    expect(result.status).toBe("mismatch_party");
    expect(result.reason).toContain("sender");
  });

  it("sets isLate flag for late payments", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ dueAt: 1000 }));
    const result = r.ingest(makeEvent({ timestamp: 2000 }));
    expect(result.status).toBe("matched");
    expect(result.isLate).toBe(true);
  });

  it("rejects expired payments when rejectExpired=true", () => {
    const r = new Reconciler({ rejectExpired: true });
    r.expect(makeExpected({ dueAt: 1000 }));
    const result = r.ingest(makeEvent({ timestamp: 2000 }));
    expect(result.status).toBe("expired");
  });

  it("applies amount tolerance in basis points", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 10_000_000n }));
    // pay 99% = 9_900_000 → within 1% tolerance
    const result = r.ingest(makeEvent({ amount: 9_900_000n }));
    expect(result.status).toBe("matched");
  });

  it("rejects underpayment beyond tolerance", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 10_000_000n }));
    // pay 98% = 9_800_000 → beyond 1% tolerance
    const result = r.ingest(makeEvent({ amount: 9_800_000n }));
    expect(result.status).toBe("mismatch_amount");
  });

  it("tolerance rounds up for small amounts", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 50n }));
    // 1% of 50 = 0.5, rounds up to 1 → underpay by 1 is within tolerance
    const result = r.ingest(makeEvent({ amount: 49n }));
    expect(result.status).toBe("matched");
  });

  it("tolerance is at least 1 when bps > 0 and amount > 0", () => {
    const r = new Reconciler({ amountToleranceBps: 1 }); // 0.01%
    r.expect(makeExpected({ amount: 1n }));
    // 0.01% of 1 = 0.0001, rounds up to 1 → underpay by 1 is within tolerance
    const result = r.ingest(makeEvent({ amount: 0n }));
    expect(result.status).toBe("matched");
  });

  it("tolerance is 0 when bps is 0", () => {
    const r = new Reconciler({ amountToleranceBps: 0 });
    r.expect(makeExpected({ amount: 50n }));
    // exact amount required
    const result = r.ingest(makeEvent({ amount: 49n }));
    expect(result.status).toBe("mismatch_amount");
  });

  it("filters by issuerTag", () => {
    const otherTag = issuerTagFromNamespace("other-app");
    const r = new Reconciler({ issuerTag: TAG });
    const otherMemo = encodeMemoV1({
      type: "invoice",
      issuerTag: otherTag,
      ulid: "01MASW9NF6YW40J40H289H858P",
    });
    r.expect(makeExpected());
    const result = r.ingest(makeEvent({ memoRaw: otherMemo }));
    expect(result.status).toBe("unknown_memo");
  });

  it("throws on duplicate expect()", () => {
    const r = new Reconciler();
    const exp = makeExpected();
    r.expect(exp);
    expect(() => r.expect(exp)).toThrow("already registered");
  });

  it("removeExpected returns true if existed", () => {
    const r = new Reconciler();
    const exp = makeExpected();
    r.expect(exp);
    expect(r.removeExpected(exp.memoRaw)).toBe(true);
    expect(r.removeExpected(exp.memoRaw)).toBe(false);
  });

  it("removeExpected cleans up partial accumulation", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // Ingest partial payment
    const p1 = r.ingest(makeEvent({ memoRaw: memo, amount: 3_000_000n }));
    expect(p1.status).toBe("partial");
    expect(p1.remainingAmount).toBe(7_000_000n);

    // Remove and re-register
    r.removeExpected(memo);
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // New partial should start from scratch, not include old 3M
    const p2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 4_000_000n,
        txHash: "0x2222000000000000000000000000000000000000000000000000000000000002",
        logIndex: 0,
      }),
    );
    expect(p2.status).toBe("partial");
    expect(p2.remainingAmount).toBe(6_000_000n); // 10M - 4M, not 10M - 7M
  });

  it("ingestMany processes all events", () => {
    const r = new Reconciler();
    r.expect(makeExpected());
    const results = r.ingestMany([
      makeEvent(),
      makeEvent({
        memoRaw: undefined,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    ]);
    expect(results).toHaveLength(2);
    expect(results[0]!.status).toBe("matched");
    expect(results[1]!.status).toBe("no_memo");
  });

  it("report() returns correct summary", () => {
    const r = new Reconciler();
    const memo1 = makeMemo("01MASW9NF6YW40J40H289H858P");
    const memo2 = makeMemo("01MASW9NF6YW40J40H289H8580");

    r.expect(makeExpected({ memoRaw: memo1, amount: 10_000_000n }));
    r.expect(makeExpected({ memoRaw: memo2, amount: 20_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo1, amount: 10_000_000n }));
    r.ingest(
      makeEvent({
        memoRaw: undefined,
        txHash: "0xcccc000000000000000000000000000000000000000000000000000000000003",
      }),
    );

    const report = r.report();
    expect(report.matched).toHaveLength(1);
    expect(report.issues).toHaveLength(1);
    expect(report.pending).toHaveLength(1);
    expect(report.pending[0]!.memoRaw).toBe(memo2);
    expect(report.summary.matchedCount).toBe(1);
    expect(report.summary.noMemoCount).toBe(1);
    expect(report.summary.pendingCount).toBe(1);
    expect(report.summary.totalExpected).toBe(2);
    expect(report.summary.totalExpectedAmount).toBe(30_000_000n);
    // two events ingested: memo1 (10_000_000n) + no_memo (10_000_000n default)
    expect(report.summary.totalReceivedAmount).toBe(20_000_000n);
    expect(report.summary.totalMatchedAmount).toBe(10_000_000n);
    expect(report.summary.issueCount).toBe(1);
  });

  it("reset() clears everything", () => {
    const r = new Reconciler();
    r.expect(makeExpected());
    r.ingest(makeEvent());
    r.reset();
    const report = r.report();
    expect(report.matched).toHaveLength(0);
    expect(report.pending).toHaveLength(0);
  });

  it("case-insensitive address comparison", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ to: "0xABCDEF1234567890ABCDEF1234567890ABCDEF12" as `0x${string}` }));
    const result = r.ingest(
      makeEvent({ to: "0xabcdef1234567890abcdef1234567890abcdef12" as `0x${string}` }),
    );
    expect(result.status).toBe("matched");
  });
});

describe("Reconciler: partial payments", () => {
  it("returns partial for underpayment when allowPartial=true", () => {
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 4_000_000n }));
    expect(result.status).toBe("partial");
    expect(result.remainingAmount).toBe(6_000_000n);
  });

  it("matches when two partials sum to expected", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    const r1 = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    expect(r1.status).toBe("partial");
    expect(r1.remainingAmount).toBe(6_000_000n);

    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 6_000_000n,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
        logIndex: 0,
      }),
    );
    expect(r2.status).toBe("matched");
    expect(r2.overpaidBy).toBeUndefined();
  });

  it("cleans up partial accumulation after match", () => {
    const store = new InMemoryStore();
    const memo = makeMemo("01PARTCEAN0TEST00000000000");
    const r = new Reconciler({ allowPartial: true, store });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    expect(store.getPartialTotal(memo)).toBe(4_000_000n);

    r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 6_000_000n,
        txHash: "0xcccc000000000000000000000000000000000000000000000000000000000003",
      }),
    );
    // After match, partial accumulation should be cleaned up
    expect(store.getPartialTotal(memo)).toBe(0n);
  });

  it("matches with overpay when partials exceed expected", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo, amount: 7_000_000n }));
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 5_000_000n,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
        logIndex: 0,
      }),
    );
    expect(r2.status).toBe("matched");
    expect(r2.overpaidBy).toBe(2_000_000n);
  });

  it("returns mismatch_amount when allowPartial=false (default)", () => {
    const r = new Reconciler();
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 4_000_000n }));
    expect(result.status).toBe("mismatch_amount");
    expect(result.remainingAmount).toBeUndefined();
  });

  it("report includes partialCount", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo, amount: 3_000_000n }));
    const report = r.report();
    expect(report.summary.partialCount).toBe(1);
  });

  it("partial respects tolerance for full payments", () => {
    const r = new Reconciler({ allowPartial: true, amountToleranceBps: 100 });
    r.expect(makeExpected({ amount: 10_000_000n }));
    // pay exactly expected → matched (not partial)
    const result = r.ingest(makeEvent({ amount: 10_000_000n }));
    expect(result.status).toBe("matched");
  });

  it("partial state preserved after wrong token payment", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // First partial: 4M correct
    const r1 = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    expect(r1.status).toBe("partial");
    expect(r1.remainingAmount).toBe(6_000_000n);

    // Second: wrong token → mismatch_token
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 6_000_000n,
        token: "0x9999999999999999999999999999999999999999" as const,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(r2.status).toBe("mismatch_token");

    // Third: correct token, 6M → completes match (partial 4M preserved)
    const r3 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 6_000_000n,
        txHash: "0xcccc000000000000000000000000000000000000000000000000000000000003",
      }),
    );
    expect(r3.status).toBe("matched");
  });
});

describe("Reconciler: partialToleranceMode", () => {
  it("final mode: accepts any partial, tolerance on cumulative", () => {
    const memo = makeMemo();
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 100, // 1% = 100_000 tolerance
      partialToleranceMode: "final",
    });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // 40% payment → accepted as partial (no per-payment tolerance check)
    const r1 = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    expect(r1.status).toBe("partial");
    expect(r1.remainingAmount).toBe(6_000_000n);
  });

  it("final mode: tolerance on cumulative match threshold", () => {
    const memo = makeMemo();
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 100, // 1% = 100_000 tolerance
      partialToleranceMode: "final",
    });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // 4M + 5.9M = 9.9M which is >= 10M - 100K = 9.9M threshold → matched
    r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 5_900_000n,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(r2.status).toBe("matched");
  });

  it("each mode: rejects partial beyond tolerance", () => {
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 100, // 1%
      partialToleranceMode: "each",
    });
    r.expect(makeExpected({ amount: 10_000_000n }));

    // 40% payment → underpaid by 6M > tolerance 100K → mismatch_amount
    const result = r.ingest(makeEvent({ amount: 4_000_000n }));
    expect(result.status).toBe("mismatch_amount");
  });

  it("each mode: accepts partial within tolerance", () => {
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 100, // 1% = 100_000 tolerance
      partialToleranceMode: "each",
    });
    r.expect(makeExpected({ amount: 10_000_000n }));

    // 99.5% payment → underpaid by 50K < tolerance 100K → matched
    const result = r.ingest(makeEvent({ amount: 9_950_000n }));
    expect(result.status).toBe("matched");
  });

  it("defaults to final mode", () => {
    const memo = makeMemo();
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 100,
    });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // 40% payment → should be partial (final mode default, no per-payment check)
    const result = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    expect(result.status).toBe("partial");
  });
});

describe("Reconciler: double-match prevention", () => {
  it("prevents second match on same memoRaw after direct match", () => {
    const memo = makeMemo();
    const r = new Reconciler();
    r.expect(makeExpected({ memoRaw: memo }));

    const result1 = r.ingest(makeEvent({ memoRaw: memo }));
    expect(result1.status).toBe("matched");

    const result2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(result2.status).toBe("unknown_memo");
  });

  it("prevents second match after partial-to-matched", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n }));
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 6_000_000n,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(r2.status).toBe("matched");

    const r3 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 1_000_000n,
        txHash: "0xcccc000000000000000000000000000000000000000000000000000000000003",
      }),
    );
    expect(r3.status).toBe("unknown_memo");
  });

  it("report() shows correct totalExpected after matches are removed", () => {
    const memo1 = makeMemo("01MASW9NF6YW40J40H289H858P");
    const memo2 = makeMemo("01MASW9NF6YW40J40H289H8580");
    const r = new Reconciler();

    r.expect(makeExpected({ memoRaw: memo1, amount: 10_000_000n }));
    r.expect(makeExpected({ memoRaw: memo2, amount: 20_000_000n }));

    r.ingest(makeEvent({ memoRaw: memo1, amount: 10_000_000n }));

    const report = r.report();
    expect(report.summary.totalExpected).toBe(2);
    expect(report.summary.totalExpectedAmount).toBe(30_000_000n);
    expect(report.summary.totalReceivedAmount).toBe(10_000_000n);
    expect(report.summary.totalMatchedAmount).toBe(10_000_000n);
    expect(report.pending).toHaveLength(1);
    expect(report.pending[0]!.memoRaw).toBe(memo2);
  });

  it("handles amount: 0n expected and received", () => {
    const r = new Reconciler();
    const memo = makeMemo("01AAAAAAAAAAAAAAAAAAAAAAA0");
    r.expect(makeExpected({ memoRaw: memo, amount: 0n }));
    const result = r.ingest(makeEvent({ memoRaw: memo, amount: 0n }));
    expect(result.status).toBe("matched");
  });

  it("handles very large bigint amounts", () => {
    const r = new Reconciler();
    const memo = makeMemo("01BBBBBBBBBBBBBBBBBBBBBBB0");
    const large = 2n ** 64n - 1n; // near max uint64
    r.expect(makeExpected({ memoRaw: memo, amount: large }));
    const result = r.ingest(makeEvent({ memoRaw: memo, amount: large }));
    expect(result.status).toBe("matched");
    expect(result.overpaidBy).toBeUndefined();
  });
});

describe("Reconciler: expiry + partial interactions", () => {
  it("rejects expired partial when rejectExpired=true", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true, rejectExpired: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n, dueAt: 1000 }));

    const result = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n, timestamp: 2000 }));
    expect(result.status).toBe("expired");
    expect(result.isLate).toBe(true);
  });

  it("accumulates late partial when rejectExpired=false", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true, rejectExpired: false });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n, dueAt: 1000 }));

    const r1 = r.ingest(makeEvent({ memoRaw: memo, amount: 4_000_000n, timestamp: 2000 }));
    expect(r1.status).toBe("partial");
    expect(r1.isLate).toBe(true);
    expect(r1.remainingAmount).toBe(6_000_000n);
  });

  it("matches via partials with isLate flag when payment is late", () => {
    const memo = makeMemo();
    const r = new Reconciler({ allowPartial: true, rejectExpired: false });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n, dueAt: 1000 }));

    r.ingest(makeEvent({ memoRaw: memo, amount: 6_000_000n, timestamp: 500 }));
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 4_000_000n,
        timestamp: 2000,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(r2.status).toBe("matched");
    expect(r2.isLate).toBe(true);
  });

  it("expired check runs before amount check", () => {
    const memo = makeMemo();
    const r = new Reconciler({ rejectExpired: true });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n, dueAt: 1000 }));

    // Both expired AND wrong amount — expiry should take priority
    const result = r.ingest(makeEvent({ memoRaw: memo, amount: 5_000_000n, timestamp: 2000 }));
    expect(result.status).toBe("expired");
  });
});

describe("Reconciler: tolerance boundary conditions", () => {
  it("tolerance exact boundary matches", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 10_000_000n }));
    // 1% of 10M = 100K. Pay exactly 9.9M = boundary → matched
    const result = r.ingest(makeEvent({ amount: 9_900_000n }));
    expect(result.status).toBe("matched");
  });

  it("tolerance one below boundary fails", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 10_000_000n }));
    // Pay 9_899_999 → 1 below the 9.9M threshold → mismatch
    const result = r.ingest(makeEvent({ amount: 9_899_999n }));
    expect(result.status).toBe("mismatch_amount");
  });

  it("tolerance with overpay still matches", () => {
    const r = new Reconciler({ amountToleranceBps: 100 }); // 1%
    r.expect(makeExpected({ amount: 10_000_000n }));
    // 12M overpay with tolerance → still matched
    const result = r.ingest(makeEvent({ amount: 12_000_000n }));
    expect(result.status).toBe("matched");
    expect(result.overpaidBy).toBe(2_000_000n);
  });

  it("100% tolerance allows zero payment", () => {
    const r = new Reconciler({ amountToleranceBps: 10000 }); // 100%
    r.expect(makeExpected({ amount: 10_000_000n }));
    const result = r.ingest(makeEvent({ amount: 0n }));
    expect(result.status).toBe("matched");
  });

  it("tolerance + allowPartial final mode: cumulative threshold includes tolerance", () => {
    const memo = makeMemo();
    const r = new Reconciler({
      allowPartial: true,
      amountToleranceBps: 500, // 5%
      partialToleranceMode: "final",
    });
    r.expect(makeExpected({ memoRaw: memo, amount: 10_000_000n }));

    // 5% of 10M = 500K. Threshold = 10M - 500K = 9.5M
    // First partial: 5M
    r.ingest(makeEvent({ memoRaw: memo, amount: 5_000_000n }));
    // Second partial: 4.5M → cumulative = 9.5M >= 9.5M threshold → matched
    const r2 = r.ingest(
      makeEvent({
        memoRaw: memo,
        amount: 4_500_000n,
        txHash: "0xbbbb000000000000000000000000000000000000000000000000000000000002",
      }),
    );
    expect(r2.status).toBe("matched");
  });
});
