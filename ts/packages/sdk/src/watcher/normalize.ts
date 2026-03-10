import type { PaymentEvent } from "../types";
import { decodeMemo } from "../memo/decode";

const ZERO_ADDRESS = `0x${"0".repeat(40)}` as `0x${string}`;

type TransferArgs = {
  from?: `0x${string}`;
  to?: `0x${string}`;
  /** TransferWithMemo uses `amount` */
  amount?: bigint;
  /** Plain Transfer uses `value` */
  value?: bigint;
  memo?: `0x${string}`;
};

/**
 * @internal Not exported from the public watcher API.
 *
 * Normalize a raw viem contract event log into a `PaymentEvent`.
 *
 * Handles both `TransferWithMemo` (has `amount` + `memo` args) and plain
 * `Transfer` (has `value` arg, no memo) event shapes.
 *
 * NOTE: `PaymentEvent.timestamp` is NOT populated by this function.
 * The watcher does not fetch block timestamps. If you need timestamps
 * (e.g. for reconciler expiry checks), enrich events after receiving them.
 *
 * @param log - Raw log object from viem watchContractEvent
 * @param chainId - Chain ID to tag the event with
 * @param token - Token contract address
 * @returns Normalized PaymentEvent
 */
export function normalizeLog(
  log: {
    args: TransferArgs;
    blockNumber: bigint;
    transactionHash: `0x${string}`;
    logIndex: number;
  },
  chainId: number,
  token: `0x${string}`,
): PaymentEvent {
  const { args } = log;
  const from = args.from ?? ZERO_ADDRESS;
  const to = args.to ?? ZERO_ADDRESS;
  // TransferWithMemo uses `amount`, Transfer uses `value`. Default 0n for edge cases.
  const amount = BigInt(args.amount ?? args.value ?? 0n);
  const memoRaw = args.memo;

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
