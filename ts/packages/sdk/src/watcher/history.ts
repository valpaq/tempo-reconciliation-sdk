import { createPublicClient, http } from "viem";
import type { HistoryOptions, PaymentEvent } from "../types";
import { transferWithMemoAbi, transferAbi } from "./abi";
import { normalizeLog } from "./normalize";
import { buildAddressFilter } from "./utils";

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
    const batchEnd = start + BigInt(batchSize) - 1n;
    const end = batchEnd > toBlock ? toBlock : batchEnd;

    try {
      const logArgs = {
        address: token,
        args: buildAddressFilter(to, from),
        fromBlock: start,
        toBlock: end,
      };

      const [logs, transferLogs] = await Promise.all([
        client.getLogs({ ...logArgs, event: transferWithMemoAbi[0] }),
        includeTransferOnly
          ? client.getLogs({ ...logArgs, event: transferAbi[0] })
          : Promise.resolve([]),
      ]);

      for (const log of logs) {
        const key = `${log.transactionHash}:${log.logIndex}`.toLowerCase();
        if (!seen.has(key)) {
          seen.add(key);
          events.push(normalizeLog(log, chainId, token));
        }
      }

      for (const log of transferLogs) {
        const key = `${log.transactionHash}:${log.logIndex}`.toLowerCase();
        if (!seen.has(key)) {
          seen.add(key);
          events.push(normalizeLog(log, chainId, token));
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
