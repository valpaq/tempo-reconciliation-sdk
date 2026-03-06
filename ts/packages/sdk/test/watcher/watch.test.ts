import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import type { WatchOptions } from "../../src/types";

vi.mock("viem", () => {
  const mockGetBlockNumber = vi.fn();
  const mockGetLogs = vi.fn();
  const mockClient = { getBlockNumber: mockGetBlockNumber, getLogs: mockGetLogs };
  return {
    createPublicClient: vi.fn().mockReturnValue(mockClient),
    http: vi.fn(),
    __mockClient: mockClient,
  };
});

const TOKEN: `0x${string}` = "0x20C0000000000000000000000000000000000000";
const FROM_ADDR: `0x${string}` = "0x1111111111111111111111111111111111111111";
const TO_ADDR: `0x${string}` = "0x2222222222222222222222222222222222222222";

const mockLog = {
  transactionHash: "0xabc123" as `0x${string}`,
  logIndex: 0,
  blockNumber: 100n,
  args: {
    from: FROM_ADDR,
    to: TO_ADDR,
    amount: 10_000_000n,
    memo: "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000" as `0x${string}`,
  },
};

function makeOptions(overrides: Partial<WatchOptions> = {}): WatchOptions {
  return {
    rpcUrl: "https://rpc.moderato.tempo.xyz",
    chainId: 42431,
    token: TOKEN,
    pollIntervalMs: 1000,
    ...overrides,
  };
}

async function getViemMocks() {
  const viem = await import("viem");
  const client = (
    viem as unknown as {
      __mockClient: { getBlockNumber: ReturnType<typeof vi.fn>; getLogs: ReturnType<typeof vi.fn> };
    }
  ).__mockClient;
  return {
    createPublicClient: viem.createPublicClient as ReturnType<typeof vi.fn>,
    http: viem.http as ReturnType<typeof vi.fn>,
    getBlockNumber: client.getBlockNumber,
    getLogs: client.getLogs,
  };
}

describe("watchTip20Transfers", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("creates client with http transport and correct rpcUrl", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { createPublicClient, http, getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([]);

    const stop = watchTip20Transfers(makeOptions(), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(http).toHaveBeenCalledWith("https://rpc.moderato.tempo.xyz");
    expect(createPublicClient).toHaveBeenCalledWith({
      transport: http("https://rpc.moderato.tempo.xyz"),
    });
  });

  it("calls callback for new events after first poll", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([mockLog]);

    const callback = vi.fn();
    const stop = watchTip20Transfers(makeOptions(), callback);
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(callback).toHaveBeenCalledOnce();
    const event = callback.mock.calls[0]![0];
    expect(event.txHash).toBe("0xabc123");
    expect(event.blockNumber).toBe(100n);
    expect(event.amount).toBe(10_000_000n);
    expect(event.from).toBe(FROM_ADDR);
    expect(event.to).toBe(TO_ADDR);
    expect(event.chainId).toBe(42431);
    expect(event.token).toBe(TOKEN);
  });

  it("deduplicates events with same txHash and logIndex", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([mockLog]);

    const callback = vi.fn();
    const stop = watchTip20Transfers(makeOptions(), callback);

    // First poll
    await vi.advanceTimersByTimeAsync(0);
    // Second poll after interval
    await vi.advanceTimersByTimeAsync(1000);

    stop();

    // callback should be called only once despite same log appearing in both polls
    expect(callback).toHaveBeenCalledOnce();
  });

  it("stops polling when unsubscribe is called", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([mockLog]);

    const callback = vi.fn();
    const stop = watchTip20Transfers(makeOptions(), callback);

    // First poll fires immediately
    await vi.advanceTimersByTimeAsync(0);
    const callsAfterFirstPoll = callback.mock.calls.length;

    // Stop before any more polls
    stop();

    // Advance time -- no more polls should fire
    await vi.advanceTimersByTimeAsync(5000);

    expect(callback.mock.calls.length).toBe(callsAfterFirstPoll);
    // getBlockNumber should not be called again after stop
    const getBlockNumberCallCount = getBlockNumber.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(getBlockNumber.mock.calls.length).toBe(getBlockNumberCallCount);
  });

  it("calls onError when poll throws", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber } = await getViemMocks();

    const networkError = new Error("RPC connection refused");
    getBlockNumber.mockRejectedValue(networkError);

    const onError = vi.fn();
    const stop = watchTip20Transfers(makeOptions({ onError }), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(onError).toHaveBeenCalledOnce();
    expect(onError.mock.calls[0]![0]).toBeInstanceOf(Error);
    expect(onError.mock.calls[0]![0].message).toBe("RPC connection refused");
  });

  it("wraps non-Error throws in an Error before calling onError", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber } = await getViemMocks();

    getBlockNumber.mockRejectedValue("string error");

    const onError = vi.fn();
    const stop = watchTip20Transfers(makeOptions({ onError }), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(onError).toHaveBeenCalledOnce();
    expect(onError.mock.calls[0]![0]).toBeInstanceOf(Error);
  });

  it("respects startBlock option by using it as fromBlock on first poll", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(200n);
    getLogs.mockResolvedValue([]);

    const stop = watchTip20Transfers(makeOptions({ startBlock: 150n }), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    expect(getLogs).toHaveBeenCalledOnce();
    const logsCallArgs = getLogs.mock.calls[0]![0];
    expect(logsCallArgs.fromBlock).toBe(150n);
    expect(logsCallArgs.toBlock).toBe(200n);
  });

  it("skips polling when startBlock is ahead of current block", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([]);

    const stop = watchTip20Transfers(makeOptions({ startBlock: 200n }), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    // getLogs should not be called when fromBlock > currentBlock
    expect(getLogs).not.toHaveBeenCalled();
  });

  it("fetches both TransferWithMemo and Transfer events when includeTransferOnly is true", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    const transferOnlyLog = {
      transactionHash: "0xdef456" as `0x${string}`,
      logIndex: 1,
      blockNumber: 100n,
      args: {
        from: FROM_ADDR,
        to: TO_ADDR,
        value: 5_000_000n,
      },
    };

    getBlockNumber.mockResolvedValue(100n);
    getLogs
      .mockResolvedValueOnce([mockLog]) // TransferWithMemo logs
      .mockResolvedValueOnce([transferOnlyLog]); // Transfer logs

    const callback = vi.fn();
    const stop = watchTip20Transfers(makeOptions({ includeTransferOnly: true }), callback);
    await vi.advanceTimersByTimeAsync(0);
    stop();

    // Two getLogs calls: one for TransferWithMemo, one for Transfer
    expect(getLogs).toHaveBeenCalledTimes(2);
    // Callback called once for each unique event
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it("does not fetch Transfer events when includeTransferOnly is false", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValue(100n);
    getLogs.mockResolvedValue([]);

    const stop = watchTip20Transfers(makeOptions({ includeTransferOnly: false }), vi.fn());
    await vi.advanceTimersByTimeAsync(0);
    stop();

    // Only one getLogs call for TransferWithMemo
    expect(getLogs).toHaveBeenCalledOnce();
  });

  it("deduplicates Transfer-only events across polls when includeTransferOnly is true", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    const transferLog = {
      transactionHash: "0xdef456" as `0x${string}`,
      logIndex: 0,
      blockNumber: 100n,
      args: {
        from: FROM_ADDR,
        to: TO_ADDR,
        value: 5_000_000n,
      },
    };

    getBlockNumber.mockResolvedValue(100n);
    // Both polls return the same transfer log
    getLogs
      .mockResolvedValueOnce([]) // first poll TransferWithMemo
      .mockResolvedValueOnce([transferLog]) // first poll Transfer
      .mockResolvedValueOnce([]) // second poll TransferWithMemo
      .mockResolvedValueOnce([transferLog]); // second poll Transfer

    const callback = vi.fn();
    const stop = watchTip20Transfers(makeOptions({ includeTransferOnly: true }), callback);

    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(1000);
    stop();

    // Should only fire once due to dedup
    expect(callback).toHaveBeenCalledOnce();
  });

  it("advances lastBlock after each successful poll", async () => {
    const { watchTip20Transfers } = await import("../../src/watcher/watch");
    const { getBlockNumber, getLogs } = await getViemMocks();

    getBlockNumber.mockResolvedValueOnce(100n).mockResolvedValueOnce(105n);

    getLogs.mockResolvedValue([]);

    const stop = watchTip20Transfers(makeOptions(), vi.fn());

    // First poll
    await vi.advanceTimersByTimeAsync(0);
    // Second poll
    await vi.advanceTimersByTimeAsync(1000);
    stop();

    const secondPollArgs = getLogs.mock.calls[1]![0];
    // After first poll with currentBlock=100n, lastBlock becomes 101n
    expect(secondPollArgs.fromBlock).toBe(101n);
    expect(secondPollArgs.toBlock).toBe(105n);
  });
});
