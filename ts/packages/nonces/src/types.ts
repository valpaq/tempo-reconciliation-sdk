/** Nonce concurrency strategy. */
export type NonceMode = "lanes" | "expiring";

/**
 * Lifecycle of a nonce slot. "confirmed", "failed", and "reaped" are terminal
 * transitions that immediately return the slot to "free".
 */
export type SlotState = "free" | "reserved" | "submitted";

/** A single nonce lane with its current state. */
export type NonceSlot = {
  /** Lane key (1..N for lanes mode, maxUint256 for expiring). */
  readonly nonceKey: bigint;
  /** Current sequence value within this lane. */
  nonce: bigint;
  /** Current lifecycle state. */
  state: SlotState;
  /** Unix ms when slot was reserved (undefined if free). */
  reservedAt?: number;
  /** Unix ms when tx was submitted to mempool. */
  submittedAt?: number;
  /** Transaction hash once submitted. */
  txHash?: `0x${string}`;
  /** Caller-provided idempotency key. */
  requestId?: string;
  /** Unix seconds — tx must be included before this time (expiring mode). */
  validBefore?: number;
};

/** Configuration for NoncePool. */
export type NoncePoolOptions = {
  /** Sender account address. */
  address: `0x${string}`;
  /** RPC endpoint URL for querying nonce values. */
  rpcUrl: string;
  /** Number of parallel lanes (default: 4). Only used in "lanes" mode. */
  lanes?: number;
  /** Concurrency mode (default: "lanes"). */
  mode?: NonceMode;
  /**
   * Auto-expire reservations after this duration (ms).
   * A reserved slot that is never submitted will return to "free" state.
   * Default: 30_000 (30 seconds).
   */
  reservationTtlMs?: number;
  /**
   * ValidBefore offset in seconds for expiring mode (TIP-1009).
   * Each acquired slot gets `validBefore = now + offset`.
   * Default: 30.
   */
  validBeforeOffsetS?: number;
  /** Chain ID (default: 42431 = Moderato testnet). */
  chainId?: number;
  /**
   * If true, `init()` calls `eth_chainId` on the RPC and throws if the
   * returned value does not match `chainId`. Disabled by default to avoid
   * an extra round-trip when the endpoint is known-correct.
   */
  validateChainId?: boolean;
};

/** Aggregate statistics for a NoncePool. */
export type NoncePoolStats = {
  /** Total number of managed lanes/slots. */
  total: number;
  /** Slots currently available for acquisition. */
  free: number;
  /** Slots reserved but not yet submitted. */
  reserved: number;
  /** Slots with pending transactions. */
  submitted: number;
  /** Cumulative confirmed transactions. */
  confirmed: number;
  /** Cumulative failed transactions. */
  failed: number;
  /** Cumulative reaped (expired reservation) count. */
  expired: number;
};
