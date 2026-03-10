import { createPublicClient, webSocket } from "viem";
import { transferWithMemoAbi, transferAbi } from "./abi";
import { normalizeLog } from "./normalize";
import { DedupCache } from "./dedup";
import type { WatchWsOptions, PaymentEvent } from "../types";
import { buildAddressFilter } from "./utils";

/**
 * Subscribe to TIP-20 transfer events via WebSocket.
 *
 * Deduplicates by (txHash, logIndex). Reconnects on error with exponential
 * backoff up to `maxReconnects` times.
 *
 * @param options - WebSocket URL, chain ID, token, filters, and reconnect config
 * @param callback - Called once per unique PaymentEvent
 * @returns Unsubscribe function
 *
 * @example
 * ```ts
 * import { watchTip20TransfersWs } from "@tempo-reconcile/sdk/watcher";
 *
 * const unsubscribe = watchTip20TransfersWs(
 *   {
 *     wsUrl: "wss://rpc.moderato.tempo.xyz",
 *     chainId: 42431,
 *     token: "0x20C0000000000000000000000000000000000000",
 *     onError: (err) => console.error("ws error:", err),
 *   },
 *   (event) => {
 *     console.log("transfer:", event.txHash, event.amount);
 *   },
 * );
 *
 * // Later: stop listening
 * unsubscribe();
 * ```
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
  const pruneInterval = setInterval(() => dedup.prune(), dedupeTtlMs);
  let unwatchers: (() => void)[] = [];
  let stopped = false;
  let reconnecting = false;
  let reconnectCount = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | undefined;

  /** Emit a log if not already seen. Shared by both event type handlers. */
  const emit = (txHash: `0x${string}`, logIndex: number, event: PaymentEvent): void => {
    if (dedup.has(txHash, logIndex)) return;
    dedup.add(txHash, logIndex);
    callback(event);
  };

  function connect(): void {
    if (stopped) return;

    const client = createPublicClient({
      transport: webSocket(wsUrl),
    });

    const args = buildAddressFilter(to, from);

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
      onLogs(logs) {
        reconnectCount = 0;
        for (const log of logs) {
          emit(log.transactionHash, log.logIndex, normalizeLog(log, chainId, token));
        }
      },
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
        onLogs(logs) {
          reconnectCount = 0;
          for (const log of logs) {
            emit(log.transactionHash, log.logIndex, normalizeLog(log, chainId, token));
          }
        },
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
      reconnecting = false;
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
    clearInterval(pruneInterval);
    if (reconnectTimer !== undefined) {
      clearTimeout(reconnectTimer);
      reconnectTimer = undefined;
    }
    for (const fn of unwatchers) fn();
    unwatchers = [];
  };
}
