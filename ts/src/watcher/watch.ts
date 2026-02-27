import { createPublicClient, http } from "viem";
import type { WatchOptions, PaymentEvent } from "../types";
import { transferWithMemoAbi, transferAbi } from "./abi";
import { normalizeLog } from "./normalize";
import { DedupCache } from "./dedup";

/**
 * Subscribe to TIP-20 transfer events via HTTP polling.
 *
 * Polls `eth_getLogs` on each interval, deduplicates events, and invokes callback.
 * Transient RPC errors are reported via `options.onError` and polling continues.
 *
 * @param options - RPC URL, chain ID, token, filters, and polling config
 * @param callback - Called once per unique PaymentEvent
 * @returns Unsubscribe function — call it to stop polling
 */
export function watchTip20Transfers(
  options: WatchOptions,
  callback: (event: PaymentEvent) => void,
): () => void {
  const {
    rpcUrl,
    chainId,
    token,
    to,
    from,
    pollIntervalMs = 1000,
    dedupeTtlMs = 60_000,
    includeTransferOnly,
  } = options;

  if (!rpcUrl) throw new Error("rpcUrl is required");

  const client = createPublicClient({
    transport: http(rpcUrl),
  });

  const dedup = new DedupCache(dedupeTtlMs);
  let stopped = false;
  let lastBlock: bigint | undefined = options.startBlock;
  let timeoutId: ReturnType<typeof setTimeout> | undefined;

  async function poll(): Promise<void> {
    if (stopped) return;

    try {
      const currentBlock = await client.getBlockNumber();
      const fromBlock = lastBlock ?? currentBlock;

      if (fromBlock > currentBlock) {
        schedule();
        return;
      }

      const logs = await client.getLogs({
        address: token,
        event: transferWithMemoAbi[0],
        args: {
          ...(to ? { to } : {}),
          ...(from ? { from } : {}),
        },
        fromBlock,
        toBlock: currentBlock,
      });

      for (const log of logs) {
        if (dedup.has(log.transactionHash, log.logIndex)) continue;
        dedup.add(log.transactionHash, log.logIndex);
        callback(normalizeLog(log, chainId, token));
      }

      if (includeTransferOnly) {
        const transferLogs = await client.getLogs({
          address: token,
          event: transferAbi[0],
          args: {
            ...(to ? { to } : {}),
            ...(from ? { from } : {}),
          },
          fromBlock,
          toBlock: currentBlock,
        });

        for (const log of transferLogs) {
          if (dedup.has(log.transactionHash, log.logIndex)) continue;
          dedup.add(log.transactionHash, log.logIndex);
          callback(normalizeLog(log, chainId, token));
        }
      }

      lastBlock = currentBlock + 1n;
    } catch (err) {
      options.onError?.(err instanceof Error ? err : new Error(String(err)));
    }

    schedule();
  }

  function schedule(): void {
    if (!stopped) {
      timeoutId = setTimeout(() => void poll(), pollIntervalMs);
    }
  }

  void poll();

  return () => {
    stopped = true;
    if (timeoutId) clearTimeout(timeoutId);
  };
}
