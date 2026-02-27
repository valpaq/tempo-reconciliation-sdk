import { createPublicClient, webSocket } from "viem";
import { transferWithMemoAbi, transferAbi } from "./abi";
import { normalizeLog } from "./normalize";
import { DedupCache } from "./dedup";
import type { WatchWsOptions, PaymentEvent } from "../types";

/**
 * Subscribe to TIP-20 transfer events via WebSocket (`eth_subscribe`).
 *
 * Uses viem's `watchContractEvent` with `poll: false` for push-based delivery.
 * Deduplicates events by (txHash, logIndex). Errors are forwarded to `options.onError`.
 *
 * Reconnects automatically on WebSocket errors with exponential backoff
 * (configurable via `maxReconnects` and `reconnectDelayMs`).
 *
 * @param options - WebSocket URL, chain ID, token, filters, and dedup/reconnect config
 * @param callback - Called once per unique PaymentEvent
 * @returns Unsubscribe function — call it to close subscriptions and stop reconnecting
 */
export function watchTip20TransfersWs(
  options: WatchWsOptions,
  callback: (event: PaymentEvent) => void,
): () => void {
  const {
    wsUrl,
    chainId,
    token,
    to,
    from,
    includeTransferOnly,
    dedupeTtlMs = 60_000,
    maxReconnects = 5,
    reconnectDelayMs = 1000,
  } = options;

  if (!wsUrl) throw new Error("wsUrl is required");

  const dedup = new DedupCache(dedupeTtlMs);
  let unwatchers: (() => void)[] = [];
  let stopped = false;
  let reconnecting = false;
  let reconnectCount = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined;

  function connect(): void {
    if (stopped) return;

    const client = createPublicClient({
      transport: webSocket(wsUrl),
    });

    const args = {
      ...(to ? { to } : {}),
      ...(from ? { from } : {}),
    };

    const handleLogs = (logs: readonly unknown[]): void => {
      reconnectCount = 0;
      for (const log of logs as Array<{ transactionHash: `0x${string}`; logIndex: number }>) {
        if (dedup.has(log.transactionHash, log.logIndex)) continue;
        dedup.add(log.transactionHash, log.logIndex);
        callback(normalizeLog(log as never, chainId, token));
      }
    };

    const handleError = (err: unknown): void => {
      const error = err instanceof Error ? err : new Error(String(err));
      options.onError?.(error);
      scheduleReconnect();
    };

    const unwatch1 = client.watchContractEvent({
      address: token,
      abi: transferWithMemoAbi,
      eventName: "TransferWithMemo",
      poll: false,
      args,
      onLogs: handleLogs as never,
      onError: handleError,
    });
    unwatchers.push(unwatch1);

    if (includeTransferOnly) {
      const unwatch2 = client.watchContractEvent({
        address: token,
        abi: transferAbi,
        eventName: "Transfer",
        poll: false,
        args,
        onLogs: handleLogs as never,
        onError: handleError,
      });
      unwatchers.push(unwatch2);
    }
  }

  function scheduleReconnect(): void {
    if (stopped || reconnecting) return;
    if (maxReconnects === 0) return;

    reconnecting = true;

    for (const fn of unwatchers) fn();
    unwatchers = [];

    if (reconnectCount >= maxReconnects) {
      options.onError?.(new Error(`WebSocket: max reconnects (${maxReconnects}) reached`));
      return;
    }

    reconnectCount++;
    const delay = Math.min(reconnectDelayMs * 2 ** (reconnectCount - 1), 30_000);

    reconnectTimer = setTimeout(() => {
      reconnectTimer = undefined;
      if (stopped) return;
      reconnecting = false;
      connect();
    }, delay);
  }

  connect();

  return () => {
    stopped = true;
    if (reconnectTimer !== undefined) {
      clearTimeout(reconnectTimer);
      reconnectTimer = undefined;
    }
    for (const fn of unwatchers) fn();
    unwatchers = [];
  };
}
