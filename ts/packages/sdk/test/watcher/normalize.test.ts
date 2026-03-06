import { describe, it, expect } from "vitest";
import { normalizeLog } from "../../src/watcher/normalize";
import { encodeMemoV1 } from "../../src/memo/encode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";
import { isMemoV1 } from "../../src/memo/decode";

const TAG = issuerTagFromNamespace("test-app");
const MEMO = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: "01MASW9NF6YW40J40H289H858P" });

const CHAIN_ID = 42431;
const TOKEN: `0x${string}` = "0x20C0000000000000000000000000000000000000";

type LogInput = Parameters<typeof normalizeLog>[0];

function makeLog(overrides: Partial<LogInput> = {}): LogInput {
  return {
    args: {
      from: "0x1111111111111111111111111111111111111111",
      to: "0x2222222222222222222222222222222222222222",
      amount: 5_000_000n,
      memo: MEMO,
    },
    blockNumber: 42n,
    transactionHash: "0xabcd000000000000000000000000000000000000000000000000000000000001",
    logIndex: 0,
    ...overrides,
  };
}

describe("normalizeLog", () => {
  it("maps all fields to PaymentEvent", () => {
    const event = normalizeLog(makeLog(), CHAIN_ID, TOKEN);
    expect(event.chainId).toBe(CHAIN_ID);
    expect(event.blockNumber).toBe(42n);
    expect(event.txHash).toBe("0xabcd000000000000000000000000000000000000000000000000000000000001");
    expect(event.logIndex).toBe(0);
    expect(event.token).toBe(TOKEN);
    expect(event.from).toBe("0x1111111111111111111111111111111111111111");
    expect(event.to).toBe("0x2222222222222222222222222222222222222222");
    expect(event.amount).toBe(5_000_000n);
  });

  it("decodes a valid memo", () => {
    const event = normalizeLog(makeLog(), CHAIN_ID, TOKEN);
    expect(event.memoRaw).toBe(MEMO);
    const memo = event.memo ?? null;
    if (!isMemoV1(memo)) throw new Error("expected MemoV1");
    expect(memo.t).toBe("invoice");
    expect(memo.ulid).toBe("01MASW9NF6YW40J40H289H858P");
  });

  it("sets memo to null when no memo in log", () => {
    const log = makeLog({
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        amount: 1n,
      },
    });
    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.memoRaw).toBeUndefined();
    expect(event.memo).toBeNull();
  });

  it("sets memo to null for invalid memo bytes", () => {
    const log = makeLog({
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        amount: 1n,
        memo: "0x0000000000000000000000000000000000000000000000000000000000000000",
      },
    });
    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.memoRaw).toBe(
      "0x0000000000000000000000000000000000000000000000000000000000000000",
    );
    expect(event.memo).toBeNull();
  });

  it("uses value field when amount is missing", () => {
    const log: LogInput = {
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        value: 7_000_000n,
        memo: MEMO,
      },
      blockNumber: 10n,
      transactionHash: "0xffff000000000000000000000000000000000000000000000000000000000001",
      logIndex: 3,
    };
    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.amount).toBe(7_000_000n);
  });

  it("defaults amount to 0n when both amount and value are missing", () => {
    const log: LogInput = {
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        memo: MEMO,
      },
      blockNumber: 10n,
      transactionHash: "0xffff000000000000000000000000000000000000000000000000000000000001",
      logIndex: 0,
    };
    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.amount).toBe(0n);
  });
});
