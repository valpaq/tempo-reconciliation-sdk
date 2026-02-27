import type {
  ExpectedPayment,
  MatchResult,
  MatchStatus,
  PaymentEvent,
  ReconcileReport,
  ReconcileSummary,
  ReconcilerOptions,
  ReconcileStore,
} from "../types";
import { decodeMemoV1, isMemoV1 } from "../memo/decode";
import { InMemoryStore } from "./store";

/**
 * Stateful payment reconciliation engine.
 *
 * Register expected payments with `expect()`, feed incoming chain events
 * with `ingest()`, and call `report()` to get a full reconciliation report.
 * Results are idempotent: ingesting the same event twice returns the cached result.
 */
export class Reconciler {
  private store: ReconcileStore;
  private issuerTag?: bigint;
  private strictSender: boolean;
  private allowOverpayment: boolean;
  private allowPartial: boolean;
  private rejectExpired: boolean;
  private toleranceBps: number;
  private partialToleranceMode: "final" | "each";
  private expectedCount = 0;
  private expectedTotalAmount = 0n;

  /**
   * Create a new Reconciler instance.
   *
   * @param options - Configuration: store backend, issuerTag filter, tolerance, and flags
   * @example
   * ```ts
   * const r = new Reconciler({
   *   issuerTag: issuerTagFromNamespace("my-app"),
   *   allowPartial: true,
   *   amountToleranceBps: 100, // 1%
   * });
   * ```
   */
  constructor(options?: ReconcilerOptions) {
    this.store = options?.store ?? new InMemoryStore();
    this.issuerTag = options?.issuerTag;
    this.strictSender = options?.strictSender ?? false;
    this.allowOverpayment = options?.allowOverpayment ?? true;
    this.allowPartial = options?.allowPartial ?? false;
    this.rejectExpired = options?.rejectExpired ?? false;
    this.toleranceBps = options?.amountToleranceBps ?? 0;
    this.partialToleranceMode = options?.partialToleranceMode ?? "final";
  }

  /**
   * Register a payment that is expected to arrive on-chain.
   *
   * @param payment - Expected payment with memoRaw, token, to, and amount
   * @throws If a payment with the same `memoRaw` is already registered
   * @example
   * ```ts
   * r.expect({
   *   memoRaw: encodeMemoV1({ type: "invoice", issuerTag, ulid }),
   *   token: "0x20C0000000000000000000000000000000000000",
   *   to: "0x1111111111111111111111111111111111111111",
   *   amount: 10_000_000n,
   * });
   * ```
   */
  expect(payment: ExpectedPayment): void {
    if (this.store.getExpected(payment.memoRaw)) {
      throw new Error(`Expected payment already registered: ${payment.memoRaw}`);
    }
    this.store.addExpected(payment);
    this.expectedCount++;
    this.expectedTotalAmount += payment.amount;
  }

  /**
   * Process an incoming on-chain payment event and return the match result.
   * Idempotent: re-ingesting the same (txHash, logIndex) returns the cached result.
   *
   * **Note:** If an event arrives before the corresponding `expect()` call, it is
   * cached as `unknown_memo`. Re-ingesting the same event after `expect()` returns
   * the cached result, not a re-evaluation. To handle late registrations, call
   * `expect()` before ingesting events.
   *
   * @param event - PaymentEvent from watcher or history fetch
   * @returns MatchResult with status: matched | partial | mismatch_* | unknown_memo | no_memo | expired
   */
  ingest(event: PaymentEvent): MatchResult {
    const eventKey = `${event.txHash}:${event.logIndex}`;
    const existing = this.store.getResult(eventKey);
    if (existing) {
      return existing;
    }

    if (!event.memoRaw) {
      return this.record(eventKey, { status: "no_memo", payment: event });
    }

    const memo = event.memo ?? decodeMemoV1(event.memoRaw);

    if (this.issuerTag !== undefined && isMemoV1(memo) && memo.issuerTag !== this.issuerTag) {
      return this.record(eventKey, {
        status: "unknown_memo",
        payment: event,
        reason: `issuerTag mismatch: expected ${this.issuerTag}, got ${memo.issuerTag}`,
      });
    }

    const expected = this.store.getExpected(event.memoRaw);
    if (!expected) {
      return this.record(eventKey, { status: "unknown_memo", payment: event });
    }

    if (event.token.toLowerCase() !== expected.token.toLowerCase()) {
      return this.record(eventKey, {
        status: "mismatch_token",
        payment: event,
        expected,
        reason: `wrong token: expected ${expected.token}, got ${event.token}`,
      });
    }

    if (event.to.toLowerCase() !== expected.to.toLowerCase()) {
      return this.record(eventKey, {
        status: "mismatch_party",
        payment: event,
        expected,
        reason: `wrong recipient: expected ${expected.to}, got ${event.to}`,
      });
    }

    if (
      this.strictSender &&
      expected.from &&
      event.from.toLowerCase() !== expected.from.toLowerCase()
    ) {
      return this.record(eventKey, {
        status: "mismatch_party",
        payment: event,
        expected,
        reason: `wrong sender: expected ${expected.from}, got ${event.from}`,
      });
    }

    const isLate =
      expected.dueAt !== undefined &&
      event.timestamp !== undefined &&
      event.timestamp > expected.dueAt;
    if (isLate && this.rejectExpired) {
      return this.record(eventKey, {
        status: "expired",
        payment: event,
        expected,
        isLate: true,
        reason: "payment after due date",
      });
    }

    const diff = event.amount - expected.amount;
    // Ceiling division: round up so tolerance is never less than the exact fraction.
    const toleranceAmount =
      this.toleranceBps > 0 ? (expected.amount * BigInt(this.toleranceBps) + 9999n) / 10000n : 0n;

    if (diff < 0n) {
      if (this.allowPartial) {
        const absDiff = -diff;

        if (this.partialToleranceMode === "each") {
          // "each" mode: tolerance applies per-payment.
          // Beyond tolerance → reject. Within tolerance → fall through to matched.
          if (absDiff > toleranceAmount) {
            return this.record(eventKey, {
              status: "mismatch_amount",
              payment: event,
              expected,
              reason: `underpaid by ${absDiff}`,
            });
          }
          // Within tolerance in each-mode: treat as exact match, fall through to matched path below
        } else {
          // "final" mode: accumulate partials, apply tolerance to cumulative threshold
          const cumulative = this.store.addPartial(event.memoRaw, event.amount);
          const matchThreshold = expected.amount - toleranceAmount;
          if (cumulative >= matchThreshold) {
            const over = cumulative - expected.amount;
            const result = this.record(eventKey, {
              status: "matched",
              payment: event,
              expected,
              overpaidBy: over > 0n ? over : undefined,
              isLate: isLate ? true : undefined,
            });
            this.store.removeExpected(event.memoRaw);
            this.store.removePartial(event.memoRaw);
            return result;
          }
          return this.record(eventKey, {
            status: "partial",
            payment: event,
            expected,
            remainingAmount: expected.amount - cumulative,
            isLate: isLate ? true : undefined,
          });
        }
      }

      const absDiff = -diff;
      if (absDiff > toleranceAmount) {
        return this.record(eventKey, {
          status: "mismatch_amount",
          payment: event,
          expected,
          reason: `underpaid by ${absDiff}`,
        });
      }
    }

    if (diff > 0n && !this.allowOverpayment) {
      return this.record(eventKey, {
        status: "mismatch_amount",
        payment: event,
        expected,
        reason: `overpaid by ${diff}`,
        overpaidBy: diff,
      });
    }

    const result = this.record(eventKey, {
      status: "matched",
      payment: event,
      expected,
      overpaidBy: diff > 0n ? diff : undefined,
      isLate: isLate ? true : undefined,
    });
    this.store.removeExpected(event.memoRaw);
    return result;
  }

  /**
   * Process multiple payment events and return all match results.
   *
   * @param events - Array of PaymentEvents
   * @returns Array of MatchResults in the same order as the input
   */
  ingestMany(events: PaymentEvent[]): MatchResult[] {
    return events.map((e) => this.ingest(e));
  }

  /**
   * Generate a full reconciliation report from all ingested events.
   *
   * @returns ReconcileReport with matched, issues, pending arrays and summary counts
   */
  report(): ReconcileReport {
    const allResults = this.store.getAllResults();
    const matched: MatchResult[] = [];
    const issues: MatchResult[] = [];
    const counts: Record<MatchStatus, number> = {
      matched: 0,
      partial: 0,
      unknown_memo: 0,
      no_memo: 0,
      mismatch_amount: 0,
      mismatch_token: 0,
      mismatch_party: 0,
      expired: 0,
    };

    let totalReceivedAmount = 0n;
    let totalMatchedAmount = 0n;

    for (const r of allResults) {
      counts[r.status]++;
      totalReceivedAmount += r.payment.amount;

      if (r.status === "matched") {
        matched.push(r);
        totalMatchedAmount += r.expected?.amount ?? r.payment.amount;
      } else {
        issues.push(r);
      }
    }

    // After matching, expected payments are removed from the store,
    // so getAllExpected() returns only pending (unmatched) entries.
    const pending = this.store.getAllExpected();

    const summary: ReconcileSummary = {
      totalExpected: this.expectedCount,
      totalReceived: allResults.length,
      matchedCount: counts.matched,
      issueCount: allResults.length - counts.matched,
      pendingCount: pending.length,
      totalExpectedAmount: this.expectedTotalAmount,
      totalReceivedAmount,
      totalMatchedAmount,
      unknownMemoCount: counts.unknown_memo,
      noMemoCount: counts.no_memo,
      mismatchAmountCount: counts.mismatch_amount,
      mismatchTokenCount: counts.mismatch_token,
      mismatchPartyCount: counts.mismatch_party,
      expiredCount: counts.expired,
      partialCount: counts.partial,
    };

    return { matched, issues, pending, summary };
  }

  /**
   * Remove a pending expected payment by its memo bytes.
   *
   * @param memoRaw - The `0x`-prefixed bytes32 memo of the expected payment
   * @returns `true` if the payment was found and removed, `false` if not found
   */
  removeExpected(memoRaw: `0x${string}`): boolean {
    return this.store.removeExpected(memoRaw);
  }

  /** Clear all expected payments and ingested results from the store. */
  reset(): void {
    this.store.clear();
    this.expectedCount = 0;
    this.expectedTotalAmount = 0n;
  }

  private record(key: string, result: MatchResult): MatchResult {
    this.store.addResult(key, result);
    return result;
  }
}
