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

  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined {
    return this.expected.get(memoRaw.toLowerCase());
  }

  getAllExpected(): ExpectedPayment[] {
    return [...this.expected.values()];
  }

  removeExpected(memoRaw: `0x${string}`): boolean {
    return this.expected.delete(memoRaw.toLowerCase());
  }

  /** Key format: `"txHash:logIndex"`. */
  addResult(key: string, result: MatchResult): void {
    this.results.set(key.toLowerCase(), result);
  }

  getResult(key: string): MatchResult | undefined {
    return this.results.get(key.toLowerCase());
  }

  getAllResults(): MatchResult[] {
    return [...this.results.values()];
  }

  /** Accumulates amount and returns the new cumulative total. */
  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint {
    const k = memoRaw.toLowerCase();
    const current = this.partials.get(k) ?? 0n;
    const total = current + amount;
    this.partials.set(k, total);
    return total;
  }

  /** Returns `0n` if no partials recorded. */
  getPartialTotal(memoRaw: `0x${string}`): bigint {
    return this.partials.get(memoRaw.toLowerCase()) ?? 0n;
  }

  /** Cleanup after match. */
  removePartial(memoRaw: `0x${string}`): void {
    this.partials.delete(memoRaw.toLowerCase());
  }

  clear(): void {
    this.expected.clear();
    this.results.clear();
    this.partials.clear();
  }
}
