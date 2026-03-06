/**
 * Live integration tests against Tempo Moderato testnet.
 * These hit real RPC at https://rpc.moderato.tempo.xyz
 * and verify our watcher/history/normalize code works with actual chain data.
 *
 * Skipped in CI by default -- run with:
 *   TEMPO_LIVE=1 pnpm test -- --reporter=verbose test/watcher/live-testnet.test.ts
 */
import { describe, it, expect } from "vitest";
import { getTip20TransferHistory } from "../../src/watcher/history";
import { watchTip20Transfers } from "../../src/watcher/watch";

const RPC_URL = "https://rpc.moderato.tempo.xyz";
const CHAIN_ID = 42431;
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";

// Real TransferWithMemo event observed on Moderato testnet:
// block 6504870, tx 0xba01fd25..., logIndex 183
// from 0x51881fed... to 0x4489cdb6..., amount 50_000_000, memo 0x64726f70736e6170...
const KNOWN_BLOCK = 6504870n;
const KNOWN_TX = "0xba01fd25c190087f10d6d6d921f2d4a3e0e7aafd21e92cbb7f56851060e3d3ba";

const skip = !process.env["TEMPO_LIVE"];

describe.skipIf(skip)("live testnet: getTip20TransferHistory", () => {
  it("fetches Transfer events from a known block range", async () => {
    const events = await getTip20TransferHistory({
      rpcUrl: RPC_URL,
      chainId: CHAIN_ID,
      token: PATH_USD,
      fromBlock: KNOWN_BLOCK,
      toBlock: KNOWN_BLOCK,
    });

    // Block 6504870 has at least one TransferWithMemo event
    expect(events.length).toBeGreaterThan(0);

    // All events should have the correct chain/token
    for (const ev of events) {
      expect(ev.chainId).toBe(CHAIN_ID);
      expect(ev.token.toLowerCase()).toBe(PATH_USD.toLowerCase());
      expect(ev.blockNumber).toBe(KNOWN_BLOCK);
    }
  }, 30_000);

  it("finds our known TransferWithMemo transaction", async () => {
    const events = await getTip20TransferHistory({
      rpcUrl: RPC_URL,
      chainId: CHAIN_ID,
      token: PATH_USD,
      fromBlock: KNOWN_BLOCK,
      toBlock: KNOWN_BLOCK,
    });

    const match = events.find((e) => e.txHash.toLowerCase() === KNOWN_TX.toLowerCase());
    expect(match).toBeDefined();
    if (match) {
      expect(match.from.toLowerCase()).toBe("0x51881fed631dae3f998dad2cf0c13e0a932cbb11");
      expect(match.to.toLowerCase()).toBe("0x4489cdb6f4574576058a579b86de27789c1cb8f3");
      expect(match.amount).toBe(50_000_000n);
      // memo should be present and it's not a valid v1 memo format (it's "dropsnap\r")
      expect(match.memoRaw).toBeDefined();
    }
  }, 30_000);

  it("returns empty for a block range with no events for a specific recipient", async () => {
    const events = await getTip20TransferHistory({
      rpcUrl: RPC_URL,
      chainId: CHAIN_ID,
      token: PATH_USD,
      fromBlock: KNOWN_BLOCK,
      toBlock: KNOWN_BLOCK,
      // nonexistent address, should get zero results
      to: "0x0000000000000000000000000000000000000001",
    });

    expect(events).toHaveLength(0);
  }, 30_000);

  it("handles multi-block range with batching", async () => {
    const events = await getTip20TransferHistory({
      rpcUrl: RPC_URL,
      chainId: CHAIN_ID,
      token: PATH_USD,
      fromBlock: KNOWN_BLOCK,
      toBlock: KNOWN_BLOCK + 30n,
      batchSize: 10, // force multiple batches
    });

    // Should get events from the block range (the first block alone has activity)
    expect(events.length).toBeGreaterThan(0);

    // Block numbers should be within range
    for (const ev of events) {
      expect(ev.blockNumber).toBeGreaterThanOrEqual(KNOWN_BLOCK);
      expect(ev.blockNumber).toBeLessThanOrEqual(KNOWN_BLOCK + 30n);
    }
  }, 30_000);
});

describe.skipIf(skip)("live testnet: watchTip20Transfers", () => {
  it("receives at least one event from a recent block", async () => {
    const events: Awaited<ReturnType<typeof getTip20TransferHistory>> = [];

    const unsubscribe = watchTip20Transfers(
      {
        rpcUrl: RPC_URL,
        chainId: CHAIN_ID,
        token: PATH_USD,
        startBlock: KNOWN_BLOCK,
        pollIntervalMs: 500,
      },
      (event) => {
        events.push(event);
      },
    );

    // Wait for one poll cycle to pick up the known block
    await new Promise((resolve) => setTimeout(resolve, 3000));
    unsubscribe();

    expect(events.length).toBeGreaterThan(0);
    expect(events[0]!.chainId).toBe(CHAIN_ID);
  }, 15_000);

  it("unsubscribe stops polling", async () => {
    let callCount = 0;

    const unsubscribe = watchTip20Transfers(
      {
        rpcUrl: RPC_URL,
        chainId: CHAIN_ID,
        token: PATH_USD,
        startBlock: KNOWN_BLOCK,
        pollIntervalMs: 200,
      },
      () => {
        callCount++;
      },
    );

    await new Promise((resolve) => setTimeout(resolve, 1500));
    unsubscribe();
    const countAtStop = callCount;

    await new Promise((resolve) => setTimeout(resolve, 1000));
    // No new events after unsubscribe
    expect(callCount).toBe(countAtStop);
  }, 15_000);
});

describe.skipIf(skip)("live testnet: RPC basics", () => {
  it("connects to Moderato and gets chain ID 42431", async () => {
    const { createPublicClient, http } = await import("viem");
    const client = createPublicClient({ transport: http(RPC_URL) });
    const chainId = await client.getChainId();
    expect(chainId).toBe(CHAIN_ID);
  }, 10_000);

  it("gets a recent block number", async () => {
    const { createPublicClient, http } = await import("viem");
    const client = createPublicClient({ transport: http(RPC_URL) });
    const blockNumber = await client.getBlockNumber();
    // testnet should be well past block 6M
    expect(blockNumber).toBeGreaterThan(6_000_000n);
  }, 10_000);
});
