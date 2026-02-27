import { describe, it, expect, beforeEach } from "vitest";
import { InMemoryStore } from "../../src/reconciler/store";
import type { ExpectedPayment, MatchResult, PaymentEvent } from "../../src/types";

const TOKEN = "0x20C0000000000000000000000000000000000000" as const;
const ADDR = "0x1111111111111111111111111111111111111111" as const;
const SENDER = "0x2222222222222222222222222222222222222222" as const;

const MEMO_A =
  "0x0101010101010101aabbccddeeff00112233445566778899aabbccddeeff0011" as `0x${string}`;
const MEMO_B =
  "0x0201010101010101aabbccddeeff00112233445566778899aabbccddeeff0022" as `0x${string}`;
const MEMO_C =
  "0x0301010101010101aabbccddeeff00112233445566778899aabbccddeeff0033" as `0x${string}`;

function makeExpected(overrides: Partial<ExpectedPayment> = {}): ExpectedPayment {
  return {
    memoRaw: MEMO_A,
    token: TOKEN,
    to: ADDR,
    amount: 10_000_000n,
    ...overrides,
  };
}

function makePaymentEvent(overrides: Partial<PaymentEvent> = {}): PaymentEvent {
  return {
    chainId: 42431,
    blockNumber: 100n,
    txHash: "0xaaaa000000000000000000000000000000000000000000000000000000000001",
    logIndex: 0,
    token: TOKEN,
    from: SENDER,
    to: ADDR,
    amount: 10_000_000n,
    memoRaw: MEMO_A,
    ...overrides,
  };
}

function makeResult(overrides: Partial<MatchResult> = {}): MatchResult {
  return {
    status: "matched",
    payment: makePaymentEvent(),
    expected: makeExpected(),
    ...overrides,
  };
}

describe("InMemoryStore", () => {
  let store: InMemoryStore;

  beforeEach(() => {
    store = new InMemoryStore();
  });

  describe("addExpected", () => {
    it("stores a payment that can be retrieved", () => {
      const payment = makeExpected();
      store.addExpected(payment);
      expect(store.getExpected(MEMO_A)).toEqual(payment);
    });

    it("throws when adding a duplicate memo", () => {
      store.addExpected(makeExpected());
      expect(() => store.addExpected(makeExpected())).toThrow("already registered");
    });

    it("throws with memo raw in the error message", () => {
      store.addExpected(makeExpected());
      expect(() => store.addExpected(makeExpected())).toThrow(MEMO_A);
    });

    it("allows adding different memos independently", () => {
      store.addExpected(makeExpected({ memoRaw: MEMO_A }));
      store.addExpected(makeExpected({ memoRaw: MEMO_B }));
      expect(store.getExpected(MEMO_A)).toBeDefined();
      expect(store.getExpected(MEMO_B)).toBeDefined();
    });

    it("stores all fields including optional ones", () => {
      const payment = makeExpected({
        from: SENDER,
        dueAt: 9999999,
        meta: { invoiceId: "INV-001" },
      });
      store.addExpected(payment);
      expect(store.getExpected(MEMO_A)).toEqual(payment);
    });
  });

  describe("getExpected", () => {
    it("returns undefined for unknown memo", () => {
      expect(store.getExpected(MEMO_A)).toBeUndefined();
    });

    it("returns the exact stored object reference", () => {
      const payment = makeExpected();
      store.addExpected(payment);
      expect(store.getExpected(MEMO_A)).toBe(payment);
    });

    it("returns undefined after the memo has been removed", () => {
      store.addExpected(makeExpected());
      store.removeExpected(MEMO_A);
      expect(store.getExpected(MEMO_A)).toBeUndefined();
    });
  });

  describe("getAllExpected", () => {
    it("returns empty array when no payments registered", () => {
      expect(store.getAllExpected()).toEqual([]);
    });

    it("returns all registered payments", () => {
      const p1 = makeExpected({ memoRaw: MEMO_A });
      const p2 = makeExpected({ memoRaw: MEMO_B });
      store.addExpected(p1);
      store.addExpected(p2);
      const all = store.getAllExpected();
      expect(all).toHaveLength(2);
      expect(all).toContain(p1);
      expect(all).toContain(p2);
    });

    it("does not include removed payments", () => {
      const p1 = makeExpected({ memoRaw: MEMO_A });
      const p2 = makeExpected({ memoRaw: MEMO_B });
      store.addExpected(p1);
      store.addExpected(p2);
      store.removeExpected(MEMO_A);
      const all = store.getAllExpected();
      expect(all).toHaveLength(1);
      expect(all).toContain(p2);
      expect(all).not.toContain(p1);
    });

    it("returns a new array (snapshot) each call", () => {
      store.addExpected(makeExpected());
      const first = store.getAllExpected();
      const second = store.getAllExpected();
      expect(first).not.toBe(second);
    });
  });

  describe("removeExpected", () => {
    it("returns true when the memo existed", () => {
      store.addExpected(makeExpected());
      expect(store.removeExpected(MEMO_A)).toBe(true);
    });

    it("returns false when the memo did not exist", () => {
      expect(store.removeExpected(MEMO_A)).toBe(false);
    });

    it("returns false on second removal of same memo", () => {
      store.addExpected(makeExpected());
      store.removeExpected(MEMO_A);
      expect(store.removeExpected(MEMO_A)).toBe(false);
    });

    it("allows re-adding a memo after it was removed", () => {
      store.addExpected(makeExpected());
      store.removeExpected(MEMO_A);
      expect(() => store.addExpected(makeExpected())).not.toThrow();
      expect(store.getExpected(MEMO_A)).toBeDefined();
    });

    it("does not affect other registered payments", () => {
      store.addExpected(makeExpected({ memoRaw: MEMO_A }));
      store.addExpected(makeExpected({ memoRaw: MEMO_B }));
      store.removeExpected(MEMO_A);
      expect(store.getExpected(MEMO_B)).toBeDefined();
    });
  });

  describe("addResult", () => {
    it("stores a result that can be retrieved by key", () => {
      const result = makeResult();
      store.addResult("key-1", result);
      expect(store.getResult("key-1")).toBe(result);
    });

    it("overwrites an existing result for the same key", () => {
      const first = makeResult({ status: "matched" });
      const second = makeResult({ status: "partial" });
      store.addResult("key-1", first);
      store.addResult("key-1", second);
      expect(store.getResult("key-1")).toBe(second);
    });

    it("stores results with different keys independently", () => {
      const r1 = makeResult({ status: "matched" });
      const r2 = makeResult({ status: "unknown_memo" });
      store.addResult("key-1", r1);
      store.addResult("key-2", r2);
      expect(store.getResult("key-1")).toBe(r1);
      expect(store.getResult("key-2")).toBe(r2);
    });
  });

  describe("getResult", () => {
    it("returns undefined for unknown key", () => {
      expect(store.getResult("no-such-key")).toBeUndefined();
    });

    it("returns the stored result object", () => {
      const result = makeResult();
      store.addResult("my-key", result);
      expect(store.getResult("my-key")).toEqual(result);
    });
  });

  describe("getAllResults", () => {
    it("returns empty array when no results stored", () => {
      expect(store.getAllResults()).toEqual([]);
    });

    it("returns all stored results", () => {
      const r1 = makeResult({ status: "matched" });
      const r2 = makeResult({ status: "no_memo" });
      store.addResult("k1", r1);
      store.addResult("k2", r2);
      const all = store.getAllResults();
      expect(all).toHaveLength(2);
      expect(all).toContain(r1);
      expect(all).toContain(r2);
    });

    it("reflects overwritten results accurately", () => {
      const original = makeResult({ status: "partial" });
      const updated = makeResult({ status: "matched" });
      store.addResult("k1", original);
      store.addResult("k1", updated);
      const all = store.getAllResults();
      expect(all).toHaveLength(1);
      expect(all[0]).toBe(updated);
    });

    it("returns a new array (snapshot) each call", () => {
      store.addResult("k1", makeResult());
      const first = store.getAllResults();
      const second = store.getAllResults();
      expect(first).not.toBe(second);
    });
  });

  describe("addPartial", () => {
    it("returns the initial amount on first call", () => {
      expect(store.addPartial(MEMO_A, 3_000_000n)).toBe(3_000_000n);
    });

    it("accumulates amounts across multiple calls for the same memo", () => {
      store.addPartial(MEMO_A, 3_000_000n);
      const total = store.addPartial(MEMO_A, 4_000_000n);
      expect(total).toBe(7_000_000n);
    });

    it("accumulates three separate partial payments correctly", () => {
      store.addPartial(MEMO_A, 2_000_000n);
      store.addPartial(MEMO_A, 3_000_000n);
      const total = store.addPartial(MEMO_A, 5_000_000n);
      expect(total).toBe(10_000_000n);
    });

    it("keeps partial totals separate per memo", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      store.addPartial(MEMO_B, 2_000_000n);
      expect(store.getPartialTotal(MEMO_A)).toBe(5_000_000n);
      expect(store.getPartialTotal(MEMO_B)).toBe(2_000_000n);
    });

    it("handles zero amount additions", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      const total = store.addPartial(MEMO_A, 0n);
      expect(total).toBe(5_000_000n);
    });

    it("handles large bigint amounts without overflow", () => {
      const large = 999_999_999_999_999n;
      const total = store.addPartial(MEMO_A, large);
      expect(total).toBe(large);
    });
  });

  describe("getPartialTotal", () => {
    it("returns 0n for an unknown memo", () => {
      expect(store.getPartialTotal(MEMO_A)).toBe(0n);
    });

    it("returns the current cumulative total", () => {
      store.addPartial(MEMO_A, 3_000_000n);
      store.addPartial(MEMO_A, 2_000_000n);
      expect(store.getPartialTotal(MEMO_A)).toBe(5_000_000n);
    });

    it("returns 0n after clear()", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      store.clear();
      expect(store.getPartialTotal(MEMO_A)).toBe(0n);
    });
  });

  describe("removePartial", () => {
    it("removes the partial entry for a memo", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      store.removePartial(MEMO_A);
      expect(store.getPartialTotal(MEMO_A)).toBe(0n);
    });

    it("does not affect other memos", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      store.addPartial(MEMO_B, 3_000_000n);
      store.removePartial(MEMO_A);
      expect(store.getPartialTotal(MEMO_A)).toBe(0n);
      expect(store.getPartialTotal(MEMO_B)).toBe(3_000_000n);
    });

    it("is safe to call on non-existent memo", () => {
      expect(() => store.removePartial(MEMO_A)).not.toThrow();
    });
  });

  describe("clear", () => {
    it("removes all expected payments", () => {
      store.addExpected(makeExpected({ memoRaw: MEMO_A }));
      store.addExpected(makeExpected({ memoRaw: MEMO_B }));
      store.clear();
      expect(store.getAllExpected()).toEqual([]);
    });

    it("removes all results", () => {
      store.addResult("k1", makeResult());
      store.addResult("k2", makeResult());
      store.clear();
      expect(store.getAllResults()).toEqual([]);
    });

    it("resets all partial totals to 0n", () => {
      store.addPartial(MEMO_A, 5_000_000n);
      store.addPartial(MEMO_B, 3_000_000n);
      store.clear();
      expect(store.getPartialTotal(MEMO_A)).toBe(0n);
      expect(store.getPartialTotal(MEMO_B)).toBe(0n);
    });

    it("allows adding expected payments again after clear", () => {
      store.addExpected(makeExpected());
      store.clear();
      expect(() => store.addExpected(makeExpected())).not.toThrow();
    });

    it("is safe to call on an already-empty store", () => {
      expect(() => store.clear()).not.toThrow();
      expect(store.getAllExpected()).toEqual([]);
      expect(store.getAllResults()).toEqual([]);
    });

    it("leaves the store in a fully usable state", () => {
      store.addExpected(makeExpected({ memoRaw: MEMO_A }));
      store.addResult("k1", makeResult());
      store.addPartial(MEMO_A, 1_000_000n);
      store.clear();

      store.addExpected(makeExpected({ memoRaw: MEMO_B }));
      store.addResult("k2", makeResult({ status: "no_memo" }));
      store.addPartial(MEMO_B, 2_000_000n);

      expect(store.getAllExpected()).toHaveLength(1);
      expect(store.getAllResults()).toHaveLength(1);
      expect(store.getPartialTotal(MEMO_B)).toBe(2_000_000n);
    });
  });

  describe("isolation between instances", () => {
    it("does not share state between two InMemoryStore instances", () => {
      const storeA = new InMemoryStore();
      const storeB = new InMemoryStore();

      storeA.addExpected(makeExpected({ memoRaw: MEMO_A }));
      expect(storeB.getExpected(MEMO_A)).toBeUndefined();
    });

    it("clearing one instance does not affect another", () => {
      const storeA = new InMemoryStore();
      const storeB = new InMemoryStore();

      storeA.addExpected(makeExpected({ memoRaw: MEMO_A }));
      storeB.addExpected(makeExpected({ memoRaw: MEMO_A }));

      storeA.clear();
      expect(storeB.getExpected(MEMO_A)).toBeDefined();
    });
  });

  describe("all match statuses storable as results", () => {
    const statuses = [
      "matched",
      "partial",
      "unknown_memo",
      "no_memo",
      "mismatch_amount",
      "mismatch_token",
      "mismatch_party",
      "expired",
    ] as const;

    for (const status of statuses) {
      it(`stores and retrieves result with status '${status}'`, () => {
        const result = makeResult({ status });
        store.addResult(`key-${status}`, result);
        expect(store.getResult(`key-${status}`)?.status).toBe(status);
      });
    }
  });

  describe("result with optional fields", () => {
    it("stores result with overpaidBy field", () => {
      const result = makeResult({ status: "matched", overpaidBy: 500_000n });
      store.addResult("k1", result);
      expect(store.getResult("k1")?.overpaidBy).toBe(500_000n);
    });

    it("stores result with remainingAmount field", () => {
      const result = makeResult({ status: "partial", remainingAmount: 4_000_000n });
      store.addResult("k1", result);
      expect(store.getResult("k1")?.remainingAmount).toBe(4_000_000n);
    });

    it("stores result with isLate flag", () => {
      const result = makeResult({ status: "matched", isLate: true });
      store.addResult("k1", result);
      expect(store.getResult("k1")?.isLate).toBe(true);
    });

    it("stores result with reason string", () => {
      const result = makeResult({ status: "mismatch_amount", reason: "underpaid by 5000000" });
      store.addResult("k1", result);
      expect(store.getResult("k1")?.reason).toBe("underpaid by 5000000");
    });

    it("stores result without expected payment for no_memo events", () => {
      const event = makePaymentEvent({ memoRaw: undefined });
      const result: MatchResult = { status: "no_memo", payment: event };
      store.addResult("k1", result);
      expect(store.getResult("k1")?.expected).toBeUndefined();
    });
  });

  describe("expected payment with optional fields", () => {
    it("stores and retrieves expected payment with from constraint", () => {
      const p = makeExpected({ from: SENDER });
      store.addExpected(p);
      expect(store.getExpected(MEMO_A)?.from).toBe(SENDER);
    });

    it("stores and retrieves expected payment with dueAt", () => {
      const p = makeExpected({ dueAt: 1709123456 });
      store.addExpected(p);
      expect(store.getExpected(MEMO_A)?.dueAt).toBe(1709123456);
    });

    it("stores and retrieves expected payment with meta", () => {
      const meta = { invoiceId: "INV-042", customer: "Acme Corp" };
      const p = makeExpected({ meta });
      store.addExpected(p);
      expect(store.getExpected(MEMO_A)?.meta).toEqual(meta);
    });

    it("stores multiple expected payments with different memos", () => {
      store.addExpected(makeExpected({ memoRaw: MEMO_A, amount: 10_000_000n }));
      store.addExpected(makeExpected({ memoRaw: MEMO_B, amount: 20_000_000n }));
      store.addExpected(makeExpected({ memoRaw: MEMO_C, amount: 30_000_000n }));
      expect(store.getAllExpected()).toHaveLength(3);
    });
  });
});
