import type { PaymentEvent } from "../types";
import { decodeMemo } from "../memo/decode";

export function normalizeLog(
  log: {
    args: Record<string, unknown>;
    blockNumber: bigint;
    transactionHash: `0x${string}`;
    logIndex: number;
  },
  chainId: number,
  token: `0x${string}`,
): PaymentEvent {
  const args = log.args;
  const from = args["from"] as `0x${string}`;
  const to = args["to"] as `0x${string}`;
  // TransferWithMemo uses `amount`, Transfer uses `value`. Default 0n for edge cases.
  const amount =
    (args["amount"] as bigint | undefined) ?? (args["value"] as bigint | undefined) ?? 0n;
  const memoRaw = args["memo"] as `0x${string}` | undefined;

  return {
    chainId,
    blockNumber: log.blockNumber,
    txHash: log.transactionHash,
    logIndex: log.logIndex,
    token,
    from,
    to,
    amount,
    memoRaw,
    memo: memoRaw ? decodeMemo(memoRaw) : null,
  };
}
