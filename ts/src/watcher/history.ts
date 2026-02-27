import { createPublicClient, http } from "viem";
import type { HistoryOptions, PaymentEvent } from "../types";
import { transferWithMemoAbi, transferAbi } from "./abi";
import { normalizeLog } from "./normalize";

/**
 * Fetch all TIP-20 transfer events in a block range.
 *
 * Splits the range into batches (default 2000 blocks each).
 * On RPC error, calls `options.onError` then re-throws so the caller can retry.
 *
 * @param options - RPC config, token address, block range, and optional filters
 * @returns All matching PaymentEvents in ascending block order
 * @throws If any batch RPC call fails after calling onError
 */
export async function getTip20TransferHistory(options: HistoryOptions): Promise<PaymentEvent[]> {
  const {
    rpcUrl,
    chainId,
    token,
    to,
    from,
    fromBlock,
    batchSize = 2000,
    includeTransferOnly,
    onError,
  } = options;

  if (!rpcUrl) throw new Error("rpcUrl is required");

  const client = createPublicClient({
    transport: http(rpcUrl),
  });

  const toBlock = options.toBlock ?? (await client.getBlockNumber());
  if (fromBlock > toBlock) {
    throw new Error(`fromBlock (${fromBlock}) must be <= toBlock (${toBlock})`);
  }
  const events: PaymentEvent[] = [];
  const seen = new Set<string>();

  for (let start = fromBlock; start <= toBlock; start += BigInt(batchSize)) {
    const end = start + BigInt(batchSize) - 1n > toBlock ? toBlock : start + BigInt(batchSize) - 1n;

    try {
      const logs = await client.getLogs({
        address: token,
        event: transferWithMemoAbi[0],
        args: {
          ...(to ? { to } : {}),
          ...(from ? { from } : {}),
        },
        fromBlock: start,
        toBlock: end,
      });

      for (const log of logs) {
        const key = `${log.transactionHash}:${log.logIndex}`;
        if (!seen.has(key)) {
          seen.add(key);
          events.push(normalizeLog(log, chainId, token));
        }
      }

      if (includeTransferOnly) {
        const transferLogs = await client.getLogs({
          address: token,
          event: transferAbi[0],
          args: {
            ...(to ? { to } : {}),
            ...(from ? { from } : {}),
          },
          fromBlock: start,
          toBlock: end,
        });

        for (const log of transferLogs) {
          const key = `${log.transactionHash}:${log.logIndex}`;
          if (!seen.has(key)) {
            seen.add(key);
            events.push(normalizeLog(log, chainId, token));
          }
        }
      }
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      onError?.(error);
      throw error;
    }
  }

  return events;
}
