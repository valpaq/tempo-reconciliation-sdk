import type { ExpectedPayment, MatchResult, ReconcileStore } from "../types";

/**
 * In-memory implementation of ReconcileStore using Maps.
 * Suitable for scripts and tests. For persistence, implement ReconcileStore with a database.
 */
export class InMemoryStore implements ReconcileStore {
  private expected = new Map<string, ExpectedPayment>();
  private results = new Map<string, MatchResult>();
  private partials = new Map<string, bigint>();

  /** @throws If a payment with the same `memoRaw` is already registered */
  addExpected(payment: ExpectedPayment): void {
    const key = payment.memoRaw.toLowerCase();
    if (this.expected.has(key)) {
      throw new Error(`Expected payment already registered: ${payment.memoRaw}`);
    }
    this.expected.set(key, payment);
  }

  /** Look up an expected payment by its memo bytes. */
  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined {
    return this.expected.get(memoRaw.toLowerCase());
  }

  /** Return all pending (unmatched) expected payments. */
  getAllExpected(): ExpectedPayment[] {
    return [...this.expected.values()];
  }

  /** Remove an expected payment. Returns `true` if it existed. */
  removeExpected(memoRaw: `0x${string}`): boolean {
    return this.expected.delete(memoRaw.toLowerCase());
  }

  /** Store a match result keyed by `"txHash:logIndex"`. */
  addResult(key: string, result: MatchResult): void {
    this.results.set(key.toLowerCase(), result);
  }

  /** Look up a cached match result by event key. */
  getResult(key: string): MatchResult | undefined {
    return this.results.get(key.toLowerCase());
  }

  /** Return all stored match results. */
  getAllResults(): MatchResult[] {
    return [...this.results.values()];
  }

  /** Accumulate a partial payment amount and return the new cumulative total. */
  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint {
    const k = memoRaw.toLowerCase();
    const current = this.partials.get(k) ?? 0n;
    const total = current + amount;
    this.partials.set(k, total);
    return total;
  }

  /** Get the cumulative partial payment total for a memo. Returns `0n` if none. */
  getPartialTotal(memoRaw: `0x${string}`): bigint {
    return this.partials.get(memoRaw.toLowerCase()) ?? 0n;
  }

  /** Remove the partial accumulation entry for a memo (cleanup after match). */
  removePartial(memoRaw: `0x${string}`): void {
    this.partials.delete(memoRaw.toLowerCase());
  }

  /** Clear all expected payments, results, and partial totals. */
  clear(): void {
    this.expected.clear();
    this.results.clear();
    this.partials.clear();
  }
}
