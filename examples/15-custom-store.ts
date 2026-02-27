#!/usr/bin/env npx tsx
/**
 * Example: custom ReconcileStore backed by a JSON file.
 *
 * Shows how to implement ReconcileStore for persistence:
 *   - State survives process restarts
 *   - Just implement the interface, plug into Reconciler
 *   - Same pattern works for Postgres, Redis, SQLite, etc.
 *
 * Usage: npx tsx examples/15-custom-store.ts
 */
import { writeFileSync, readFileSync, existsSync } from "fs";
import {
  Reconciler,
  encodeMemoV1,
  issuerTagFromNamespace,
} from "../ts/src/index";
import type { ReconcileStore } from "../ts/src/reconciler/store";
import type {
  ExpectedPayment,
  MatchResult,
  PaymentEvent,
} from "../ts/src/types";

type StoreData = {
  expected: Record<string, ExpectedPayment>;
  results: Record<string, MatchResult>;
  partials: Record<string, string>; // bigint serialized as string
};

class JsonFileStore implements ReconcileStore {
  private data: StoreData;
  private path: string;

  constructor(path: string) {
    this.path = path;
    this.data = this.load();
  }

  private load(): StoreData {
    if (!existsSync(this.path)) {
      return { expected: {}, results: {}, partials: {} };
    }
    const raw = readFileSync(this.path, "utf-8");
    return JSON.parse(raw, (_key, value) => {
      if (typeof value === "string" && value.startsWith("bigint:")) {
        return BigInt(value.slice(7));
      }
      return value;
    }) as StoreData;
  }

  private save(): void {
    const json = JSON.stringify(
      this.data,
      (_key, value) => {
        if (typeof value === "bigint") {
          return `bigint:${value}`;
        }
        return value;
      },
      2,
    );
    writeFileSync(this.path, json);
  }

  addExpected(payment: ExpectedPayment): void {
    if (this.data.expected[payment.memoRaw]) {
      throw new Error(
        `Expected payment already registered: ${payment.memoRaw}`,
      );
    }
    this.data.expected[payment.memoRaw] = payment;
    this.save();
  }

  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined {
    return this.data.expected[memoRaw];
  }

  getAllExpected(): ExpectedPayment[] {
    return Object.values(this.data.expected);
  }

  removeExpected(memoRaw: `0x${string}`): boolean {
    if (!this.data.expected[memoRaw]) return false;
    delete this.data.expected[memoRaw];
    this.save();
    return true;
  }

  addResult(key: string, result: MatchResult): void {
    this.data.results[key] = result;
    this.save();
  }

  getResult(key: string): MatchResult | undefined {
    return this.data.results[key];
  }

  getAllResults(): MatchResult[] {
    return Object.values(this.data.results);
  }

  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint {
    const current = BigInt(this.data.partials[memoRaw] ?? "0");
    const total = current + amount;
    this.data.partials[memoRaw] = total.toString();
    this.save();
    return total;
  }

  getPartialTotal(memoRaw: `0x${string}`): bigint {
    return BigInt(this.data.partials[memoRaw] ?? "0");
  }

  clear(): void {
    this.data = { expected: {}, results: {}, partials: {} };
    this.save();
  }
}

const STORE_PATH = "/tmp/reconcile-store-demo.json";
const ISSUER = issuerTagFromNamespace("custom-store-demo");
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";

function main() {
  console.log("=== Custom ReconcileStore (JSON file) ===\n");
  console.log(`Store path: ${STORE_PATH}\n`);

  const store = new JsonFileStore(STORE_PATH);
  const reconciler = new Reconciler({ store, allowPartial: true });

  const memo1 = encodeMemoV1({
    type: "invoice",
    issuerTag: ISSUER,
    ulid: "01JNRX0KD42T3H9XJGCH5BKRWM",
  });
  const memo2 = encodeMemoV1({
    type: "payroll",
    issuerTag: ISSUER,
    ulid: "01JNRX1AA52T3H9XJGCH5BKRWN",
  });

  reconciler.expect({
    memoRaw: memo1,
    token: PATH_USD,
    to: "0x1111111111111111111111111111111111111111",
    amount: 100_000_000n, // 100 pathUSD
    meta: { invoiceId: "INV-001", customer: "Acme Corp" },
  });

  reconciler.expect({
    memoRaw: memo2,
    token: PATH_USD,
    to: "0x1111111111111111111111111111111111111111",
    amount: 50_000_000n, // 50 pathUSD
    meta: { payrollId: "PAY-2026-02", employee: "Alice" },
  });

  console.log("Registered 2 expected payments");
  console.log(`  INV-001: 100 pathUSD (memo=${memo1.slice(0, 20)}...)`);
  console.log(`  PAY-2026-02: 50 pathUSD (memo=${memo2.slice(0, 20)}...)`);

  const event1: PaymentEvent = {
    chainId: 42431,
    blockNumber: 6500000n,
    txHash:
      "0xaaaa000000000000000000000000000000000000000000000000000000000001",
    logIndex: 0,
    token: PATH_USD,
    from: "0x2222222222222222222222222222222222222222",
    to: "0x1111111111111111111111111111111111111111",
    amount: 100_000_000n,
    memoRaw: memo1,
  };

  const event2: PaymentEvent = {
    chainId: 42431,
    blockNumber: 6500001n,
    txHash:
      "0xbbbb000000000000000000000000000000000000000000000000000000000002",
    logIndex: 0,
    token: PATH_USD,
    from: "0x3333333333333333333333333333333333333333",
    to: "0x1111111111111111111111111111111111111111",
    amount: 30_000_000n, // partial — 30 of 50
    memoRaw: memo2,
  };

  console.log("\nIngesting 2 payment events...\n");

  const r1 = reconciler.ingest(event1);
  console.log(`  Event 1: ${r1.status}`, r1.expected?.meta);

  const r2 = reconciler.ingest(event2);
  console.log(
    `  Event 2: ${r2.status} (remaining: ${Number(r2.remainingAmount ?? 0n) / 1e6} pathUSD)`,
  );

  const report = reconciler.report();
  console.log("\n--- Report ---");
  console.log(`  Matched: ${report.summary.matchedCount}`);
  console.log(`  Partial: ${report.summary.partialCount}`);
  console.log(`  Pending: ${report.summary.pendingCount}`);

  console.log(`\nState persisted to ${STORE_PATH}`);
  console.log("File contents (first 200 chars):");
  const contents = readFileSync(STORE_PATH, "utf-8");
  console.log(`  ${contents.slice(0, 200)}...`);

  console.log("\n--- Restart simulation ---");
  const store2 = new JsonFileStore(STORE_PATH);
  const pending = store2.getAllExpected();
  const results = store2.getAllResults();
  console.log(
    `Loaded from file: ${pending.length} expected, ${results.length} results`,
  );
  console.log(
    "Partial total for PAY-2026-02:",
    Number(store2.getPartialTotal(memo2)) / 1e6,
    "pathUSD",
  );

  store.clear();
  console.log("\nStore cleared.");
}

main();
