import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock viem before importing history
vi.mock("viem", () => {
  const mockClient = {
    getBlockNumber: vi.fn(),
    getLogs: vi.fn(),
  };
  return {
    createPublicClient: vi.fn().mockReturnValue(mockClient),
    http: vi.fn().mockReturnValue("http-transport"),
    __mockClient: mockClient,
  };
});

import { getTip20TransferHistory } from "../../src/watcher/history";
import { createPublicClient, http } from "viem";

const TOKEN = "0x20C0000000000000000000000000000000000000" as `0x${string}`;
const CHAIN_ID = 42431;

const mockLog = {
  transactionHash: "0xabc123" as `0x${string}`,
  logIndex: 0,
  blockNumber: 100n,
  args: {
    from: "0x1111111111111111111111111111111111111111" as `0x${string}`,
    to: "0x2222222222222222222222222222222222222222" as `0x${string}`,
    amount: 10_000_000n,
    memo: "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000" as `0x${string}`,
  },
};

const transferOnlyLog = {
  transactionHash: "0xdef456" as `0x${string}`,
  logIndex: 1,
  blockNumber: 101n,
  args: {
    from: "0x3333333333333333333333333333333333333333" as `0x${string}`,
    to: "0x4444444444444444444444444444444444444444" as `0x${string}`,
    value: 5_000_000n,
  },
};

// Access the shared mock client that createPublicClient always returns.
// The factory function in vi.mock captures it in __mockClient so we can
// reference the same object regardless of vi.clearAllMocks() calls.
const viemMod = (await import("viem")) as unknown as {
  __mockClient: { getBlockNumber: ReturnType<typeof vi.fn>; getLogs: ReturnType<typeof vi.fn> };
};
const sharedClient = viemMod.__mockClient;

describe("getTip20TransferHistory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    sharedClient.getBlockNumber.mockResolvedValue(1000n);
    sharedClient.getLogs.mockResolvedValue([]);
    (createPublicClient as ReturnType<typeof vi.fn>).mockReturnValue(sharedClient);
    (http as ReturnType<typeof vi.fn>).mockReturnValue("http-transport");
  });

  it("creates client with http transport", async () => {
    await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 0n,
      toBlock: 100n,
    });

    expect(http).toHaveBeenCalledWith("https://rpc.moderato.tempo.xyz");
    expect(createPublicClient).toHaveBeenCalledWith({ transport: "http-transport" });
  });

  it("returns empty array when no logs exist", async () => {
    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 0n,
      toBlock: 100n,
    });

    expect(events).toEqual([]);
  });

  it("returns normalized events from logs", async () => {
    sharedClient.getLogs.mockResolvedValue([mockLog]);

    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 100n,
      toBlock: 100n,
    });

    expect(events).toHaveLength(1);
    expect(events[0]!.chainId).toBe(CHAIN_ID);
    expect(events[0]!.txHash).toBe("0xabc123");
    expect(events[0]!.logIndex).toBe(0);
    expect(events[0]!.blockNumber).toBe(100n);
    expect(events[0]!.token).toBe(TOKEN);
    expect(events[0]!.from).toBe("0x1111111111111111111111111111111111111111");
    expect(events[0]!.to).toBe("0x2222222222222222222222222222222222222222");
    expect(events[0]!.amount).toBe(10_000_000n);
    expect(events[0]!.memoRaw).toBe(mockLog.args.memo);
  });

  it("batches large block ranges into multiple getLogs calls", async () => {
    await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 0n,
      toBlock: 5000n,
      batchSize: 2000,
    });

    // 0-1999, 2000-3999, 4000-5000 => 3 calls
    expect(sharedClient.getLogs).toHaveBeenCalledTimes(3);

    const calls = sharedClient.getLogs.mock.calls;
    expect(calls[0]![0].fromBlock).toBe(0n);
    expect(calls[0]![0].toBlock).toBe(1999n);
    expect(calls[1]![0].fromBlock).toBe(2000n);
    expect(calls[1]![0].toBlock).toBe(3999n);
    expect(calls[2]![0].fromBlock).toBe(4000n);
    expect(calls[2]![0].toBlock).toBe(5000n);
  });

  it("defaults toBlock to current block number", async () => {
    sharedClient.getBlockNumber.mockResolvedValue(999n);

    await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 900n,
    });

    expect(sharedClient.getBlockNumber).toHaveBeenCalled();
    const lastCall = sharedClient.getLogs.mock.calls.at(-1);
    expect(lastCall![0].toBlock).toBe(999n);
  });

  it("passes to/from filters to getLogs args", async () => {
    const to = "0x2222222222222222222222222222222222222222" as `0x${string}`;
    const from = "0x1111111111111111111111111111111111111111" as `0x${string}`;

    await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 0n,
      toBlock: 100n,
      to,
      from,
    });

    const call = sharedClient.getLogs.mock.calls[0];
    expect(call![0].args).toEqual({ to, from });
  });

  it("fetches both TransferWithMemo and Transfer when includeTransferOnly is true", async () => {
    sharedClient.getLogs.mockResolvedValueOnce([mockLog]).mockResolvedValueOnce([transferOnlyLog]);

    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 100n,
      toBlock: 101n,
      includeTransferOnly: true,
    });

    // getLogs called twice per batch: once for TransferWithMemo, once for Transfer
    expect(sharedClient.getLogs).toHaveBeenCalledTimes(2);
    expect(events).toHaveLength(2);

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const eventNames = sharedClient.getLogs.mock.calls.map((call: any[]) => call[0].event.name);
    expect(eventNames).toContain("TransferWithMemo");
    expect(eventNames).toContain("Transfer");
  });

  it("deduplicates events when same log appears in both event types", async () => {
    // Same txHash:logIndex returned by both TransferWithMemo and Transfer queries
    const sharedLog = {
      transactionHash: "0xabc123" as `0x${string}`,
      logIndex: 0,
      blockNumber: 100n,
      args: {
        from: "0x1111111111111111111111111111111111111111" as `0x${string}`,
        to: "0x2222222222222222222222222222222222222222" as `0x${string}`,
        amount: 10_000_000n,
        memo: "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000" as `0x${string}`,
      },
    };
    sharedClient.getLogs.mockResolvedValueOnce([sharedLog]).mockResolvedValueOnce([sharedLog]);

    const events = await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 100n,
      toBlock: 100n,
      includeTransferOnly: true,
    });

    expect(events).toHaveLength(1);
  });

  it("calls onError and rethrows on RPC failure", async () => {
    sharedClient.getLogs.mockRejectedValue(new Error("RPC down"));
    const onError = vi.fn();

    await expect(
      getTip20TransferHistory({
        rpcUrl: "https://rpc.moderato.tempo.xyz",
        chainId: CHAIN_ID,
        token: TOKEN,
        fromBlock: 0n,
        toBlock: 100n,
        onError,
      }),
    ).rejects.toThrow("RPC down");

    expect(onError).toHaveBeenCalledOnce();
  });

  it("only fetches TransferWithMemo by default", async () => {
    await getTip20TransferHistory({
      rpcUrl: "https://rpc.moderato.tempo.xyz",
      chainId: CHAIN_ID,
      token: TOKEN,
      fromBlock: 0n,
      toBlock: 100n,
    });

    expect(sharedClient.getLogs).toHaveBeenCalledTimes(1);
    const call = sharedClient.getLogs.mock.calls[0];
    expect(call![0].event.name).toBe("TransferWithMemo");
  });

  it("throws when fromBlock > toBlock", async () => {
    await expect(
      getTip20TransferHistory({
        rpcUrl: "https://rpc.moderato.tempo.xyz",
        chainId: CHAIN_ID,
        token: TOKEN,
        fromBlock: 200n,
        toBlock: 100n,
      }),
    ).rejects.toThrow("fromBlock (200) must be <= toBlock (100)");
  });
});
