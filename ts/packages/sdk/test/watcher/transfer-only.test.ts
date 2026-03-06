import { describe, it, expect } from "vitest";
import { normalizeLog } from "../../src/watcher/normalize";

const CHAIN_ID = 42431;
const TOKEN: `0x${string}` = "0x20C0000000000000000000000000000000000000";

type LogInput = Parameters<typeof normalizeLog>[0];

describe("Transfer-only mode", () => {
  it("normalizeLog handles Transfer events (no memo)", () => {
    const log: LogInput = {
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        value: 50_000_000n,
        // no amount, no memo -- plain Transfer event
      },
      blockNumber: 100n,
      transactionHash: "0xaabb000000000000000000000000000000000000000000000000000000000001",
      logIndex: 0,
    };

    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.amount).toBe(50_000_000n);
    expect(event.memoRaw).toBeUndefined();
    expect(event.memo).toBeNull();
  });

  it("normalizeLog uses amount over value when both present", () => {
    const log: LogInput = {
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
        amount: 100_000_000n,
        value: 50_000_000n,
      },
      blockNumber: 100n,
      transactionHash: "0xaabb000000000000000000000000000000000000000000000000000000000002",
      logIndex: 0,
    };

    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.amount).toBe(100_000_000n);
  });

  it("normalizeLog defaults amount to 0n when neither amount nor value present", () => {
    const log: LogInput = {
      args: {
        from: "0x1111111111111111111111111111111111111111",
        to: "0x2222222222222222222222222222222222222222",
      },
      blockNumber: 100n,
      transactionHash: "0xaabb000000000000000000000000000000000000000000000000000000000003",
      logIndex: 0,
    };

    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.amount).toBe(0n);
  });

  it("normalizeLog preserves all address and block fields for plain Transfer", () => {
    const log: LogInput = {
      args: {
        from: "0xaaaa000000000000000000000000000000000001",
        to: "0xbbbb000000000000000000000000000000000002",
        value: 25_000_000n,
      },
      blockNumber: 999n,
      transactionHash: "0xcccc000000000000000000000000000000000000000000000000000000000004",
      logIndex: 7,
    };

    const event = normalizeLog(log, CHAIN_ID, TOKEN);
    expect(event.chainId).toBe(CHAIN_ID);
    expect(event.token).toBe(TOKEN);
    expect(event.blockNumber).toBe(999n);
    expect(event.txHash).toBe("0xcccc000000000000000000000000000000000000000000000000000000000004");
    expect(event.logIndex).toBe(7);
    expect(event.from).toBe("0xaaaa000000000000000000000000000000000001");
    expect(event.to).toBe("0xbbbb000000000000000000000000000000000002");
  });
});

describe.skipIf(!process.env["TEMPO_LIVE"])("Transfer-only mode (live)", () => {
  it("fetches Transfer events with includeTransferOnly", async () => {
    const { getTip20TransferHistory } = await import("../../src/watcher/history");

    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: 42431,
      token: "0x20C0000000000000000000000000000000000000",
      fromBlock: 6504870n,
      toBlock: 6504900n,
      includeTransferOnly: true,
    });

    // Should have at least as many events as without the flag
    const memoEvents = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: 42431,
      token: "0x20C0000000000000000000000000000000000000",
      fromBlock: 6504870n,
      toBlock: 6504900n,
    });

    expect(events.length).toBeGreaterThanOrEqual(memoEvents.length);
  }, 30_000);

  it("Transfer events without memo have null memo field", async () => {
    const { getTip20TransferHistory } = await import("../../src/watcher/history");

    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: 42431,
      token: "0x20C0000000000000000000000000000000000000",
      fromBlock: 6504870n,
      toBlock: 6504900n,
      includeTransferOnly: true,
    });

    const noMemo = events.filter((e) => !e.memoRaw);
    for (const e of noMemo) {
      expect(e.memo).toBeNull();
    }
  }, 30_000);
});
