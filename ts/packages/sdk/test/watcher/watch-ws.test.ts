import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock viem before importing watch-ws
vi.mock("viem", () => {
  const mockUnwatch = vi.fn();
  const mockWatchContractEvent = vi.fn().mockReturnValue(mockUnwatch);
  const mockClient = {
    watchContractEvent: mockWatchContractEvent,
  };
  return {
    createPublicClient: vi.fn().mockReturnValue(mockClient),
    webSocket: vi.fn().mockReturnValue("ws-transport"),
    __mockClient: mockClient,
    __mockWatchContractEvent: mockWatchContractEvent,
    __mockUnwatch: mockUnwatch,
  };
});

import { watchTip20TransfersWs } from "../../src/watcher/watch-ws";
import { createPublicClient, webSocket } from "viem";

describe("watchTip20TransfersWs", () => {
  const TOKEN = "0x20C0000000000000000000000000000000000000" as `0x${string}`;
  const WS_URL = "wss://rpc.moderato.tempo.xyz";

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    // Re-apply the mock return values after clearing
    const mockUnwatch = vi.fn();
    const mockWatchContractEvent = vi.fn().mockReturnValue(mockUnwatch);
    (createPublicClient as ReturnType<typeof vi.fn>).mockReturnValue({
      watchContractEvent: mockWatchContractEvent,
    });
  });

  it("creates client with webSocket transport", () => {
    const cb = vi.fn();
    watchTip20TransfersWs({ wsUrl: WS_URL, chainId: 42431, token: TOKEN }, cb);

    expect(webSocket).toHaveBeenCalledWith(WS_URL);
    expect(createPublicClient).toHaveBeenCalledWith({
      transport: "ws-transport",
    });
  });

  it("subscribes to TransferWithMemo with poll: false", () => {
    const cb = vi.fn();
    watchTip20TransfersWs({ wsUrl: WS_URL, chainId: 42431, token: TOKEN }, cb);

    // Get the mock client's watchContractEvent
    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    expect(client.watchContractEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        address: TOKEN,
        eventName: "TransferWithMemo",
        poll: false,
      }),
    );
  });

  it("returns unsubscribe function", () => {
    const cb = vi.fn();
    const unsub = watchTip20TransfersWs({ wsUrl: WS_URL, chainId: 42431, token: TOKEN }, cb);
    expect(typeof unsub).toBe("function");
    unsub();
  });

  it("subscribes to both events when includeTransferOnly is set", () => {
    const cb = vi.fn();
    watchTip20TransfersWs(
      { wsUrl: WS_URL, chainId: 42431, token: TOKEN, includeTransferOnly: true },
      cb,
    );

    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    expect(client.watchContractEvent).toHaveBeenCalledTimes(2);

    const calls = client.watchContractEvent.mock.calls;
    expect(calls[0][0].eventName).toBe("TransferWithMemo");
    expect(calls[1][0].eventName).toBe("Transfer");
  });

  it("only subscribes to TransferWithMemo by default", () => {
    const cb = vi.fn();
    watchTip20TransfersWs({ wsUrl: WS_URL, chainId: 42431, token: TOKEN }, cb);

    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    // Count only calls from this test - 1 for TransferWithMemo
    const lastCall = client.watchContractEvent.mock.calls.at(-1);
    expect(lastCall[0].eventName).toBe("TransferWithMemo");
  });

  it("passes address filters to subscription args", () => {
    const cb = vi.fn();
    const to = "0x1234567890abcdef1234567890abcdef12345678" as `0x${string}`;
    watchTip20TransfersWs({ wsUrl: WS_URL, chainId: 42431, token: TOKEN, to }, cb);

    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    const lastCall = client.watchContractEvent.mock.calls.at(-1);
    expect(lastCall[0].args).toEqual({ to });
  });

  it("reconnects on WebSocket error with exponential backoff", () => {
    const onError = vi.fn();
    watchTip20TransfersWs(
      {
        wsUrl: WS_URL,
        chainId: 42431,
        token: TOKEN,
        onError,
        maxReconnects: 3,
        reconnectDelayMs: 100,
      },
      vi.fn(),
    );

    expect(createPublicClient).toHaveBeenCalledTimes(1);

    // Trigger onError from the subscription
    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    const onErrorCb = client.watchContractEvent.mock.calls[0][0].onError;
    onErrorCb(new Error("connection lost"));

    // First reconnect after 100ms
    vi.advanceTimersByTime(100);
    expect(createPublicClient).toHaveBeenCalledTimes(2);

    // Trigger another error
    const client2 = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    const onErrorCb2 = client2.watchContractEvent.mock.calls[0][0].onError;
    onErrorCb2(new Error("connection lost again"));

    // Second reconnect after 200ms (exponential)
    vi.advanceTimersByTime(200);
    expect(createPublicClient).toHaveBeenCalledTimes(3);
  });

  it("stops reconnecting after maxReconnects is exhausted", () => {
    const onError = vi.fn();
    watchTip20TransfersWs(
      {
        wsUrl: WS_URL,
        chainId: 42431,
        token: TOKEN,
        onError,
        maxReconnects: 2,
        reconnectDelayMs: 100,
      },
      vi.fn(),
    );

    // First error -> reconnect attempt 1
    const client1 = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    client1.watchContractEvent.mock.calls[0][0].onError(new Error("err"));
    vi.advanceTimersByTime(100);
    expect(createPublicClient).toHaveBeenCalledTimes(2);

    // Second error -> reconnect attempt 2
    const client2 = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    client2.watchContractEvent.mock.calls[0][0].onError(new Error("err"));
    vi.advanceTimersByTime(200);
    expect(createPublicClient).toHaveBeenCalledTimes(3);

    // Third error -> max reached, no more reconnects
    const client3 = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    client3.watchContractEvent.mock.calls[0][0].onError(new Error("err"));
    vi.advanceTimersByTime(60_000);
    expect(createPublicClient).toHaveBeenCalledTimes(3); // unchanged

    // Verify the "max reconnects reached" message was emitted
    expect(onError).toHaveBeenCalledWith(
      expect.objectContaining({ message: "WebSocket: max reconnects (2) reached" }),
    );
  });

  it("does not reconnect when maxReconnects is 0", () => {
    const onError = vi.fn();
    watchTip20TransfersWs(
      { wsUrl: WS_URL, chainId: 42431, token: TOKEN, onError, maxReconnects: 0 },
      vi.fn(),
    );

    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    client.watchContractEvent.mock.calls[0][0].onError(new Error("err"));

    vi.advanceTimersByTime(60_000);
    // No reconnect — still just 1 client creation
    expect(createPublicClient).toHaveBeenCalledTimes(1);
  });

  it("stops reconnecting when unsubscribed", () => {
    const onError = vi.fn();
    const unsub = watchTip20TransfersWs(
      {
        wsUrl: WS_URL,
        chainId: 42431,
        token: TOKEN,
        onError,
        maxReconnects: 5,
        reconnectDelayMs: 100,
      },
      vi.fn(),
    );

    // Trigger error, starting reconnect timer
    const client = (createPublicClient as ReturnType<typeof vi.fn>).mock.results.at(-1)?.value;
    client.watchContractEvent.mock.calls[0][0].onError(new Error("err"));

    // Unsubscribe before timer fires
    unsub();
    vi.advanceTimersByTime(60_000);

    // No reconnect happened
    expect(createPublicClient).toHaveBeenCalledTimes(1);
  });
});
