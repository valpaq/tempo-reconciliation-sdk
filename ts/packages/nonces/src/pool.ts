import { createPublicClient, http, isAddress } from "viem";
import type { PublicClient } from "viem";
import type { NonceSlot, NoncePoolOptions, NoncePoolStats, NonceMode } from "./types";
import { getNonceFromPrecompile } from "./rpc";
import {
  MODERATO_CHAIN_ID,
  DEFAULT_LANES,
  DEFAULT_RESERVATION_TTL_MS,
  DEFAULT_VALID_BEFORE_OFFSET_S,
  MAX_UINT256,
} from "./constants";

/**
 * Nonce pool for Tempo's 2D nonce system.
 * Lifecycle: `free → reserved → submitted → confirmed → free`.
 */
export class NoncePool {
  private readonly address: `0x${string}`;
  private readonly rpcUrl: string;
  private readonly mode: NonceMode;
  private readonly laneCount: number;
  private readonly reservationTtlMs: number;
  private readonly validBeforeOffsetS: number;
  // Prefixed to avoid collision with the public chainId getter below.
  private readonly _chainId: number;
  private readonly validateChainId: boolean;

  private client: PublicClient | null = null;
  private slots: Map<bigint, NonceSlot> = new Map();
  private initialized = false;
  private initializing = false;
  private confirmedCount = 0;
  private failedCount = 0;
  private reapedCount = 0;

  constructor(options: NoncePoolOptions) {
    if (!isAddress(options.address)) throw new Error("NoncePool: invalid address format");
    if (!options.rpcUrl) throw new Error("NoncePool: rpcUrl is required");

    if (options.mode === "expiring" && options.lanes && options.lanes > 1) {
      throw new Error("NoncePool: lanes option is not supported in expiring mode");
    }

    this.address = options.address;
    this.rpcUrl = options.rpcUrl;
    this.mode = options.mode ?? "lanes";
    this.laneCount = options.lanes ?? DEFAULT_LANES;
    this.reservationTtlMs = options.reservationTtlMs ?? DEFAULT_RESERVATION_TTL_MS;
    this.validBeforeOffsetS = options.validBeforeOffsetS ?? DEFAULT_VALID_BEFORE_OFFSET_S;
    this._chainId = options.chainId ?? MODERATO_CHAIN_ID;
    this.validateChainId = options.validateChainId ?? false;

    if (!Number.isInteger(this.laneCount) || this.laneCount < 1)
      throw new Error("NoncePool: lanes must be an integer >= 1");
    if (this.reservationTtlMs <= 0) throw new Error("NoncePool: reservationTtlMs must be > 0");
    if (this.validBeforeOffsetS <= 0) throw new Error("NoncePool: validBeforeOffsetS must be > 0");
  }

  /** Chain ID this pool is configured for. */
  get chainId(): number {
    return this._chainId;
  }

  /**
   * Initialize the pool by querying on-chain nonce values.
   * Must be called before `acquire()`.
   *
   * **Note:** The `chainId` option is tracked locally and is not validated
   * against the RPC endpoint unless `validateChainId: true` is passed to the
   * constructor. Without validation, a mismatched `rpcUrl` silently returns
   * incorrect nonce values.
   *
   * @throws If already initialized (call `reset()` to re-sync)
   * @throws If initialization is already in progress (concurrent call)
   * @throws If `validateChainId` is true and the RPC chain ID does not match
   */
  async init(): Promise<void> {
    if (this.initialized) {
      throw new Error("NoncePool: already initialized — call reset() to re-sync nonces");
    }
    if (this.initializing) {
      throw new Error("NoncePool: initialization already in progress");
    }
    this.initializing = true;
    try {
      this.client = createPublicClient({ transport: http(this.rpcUrl) });
      if (this.validateChainId) {
        const rpcChainId = await this.client.getChainId();
        if (rpcChainId !== this._chainId) {
          throw new Error(
            `NoncePool: chainId mismatch — configured ${this._chainId}, RPC returned ${rpcChainId}`,
          );
        }
      }
      this.slots = this._slotsToMap(await this._fetchInitialSlots());
      this.initialized = true;
    } finally {
      this.initializing = false;
    }
  }

  /**
   * Reserve the next free slot. O(N) linear scan over slots.
   *
   * @param requestId - Optional idempotency key. If a reserved or submitted slot with this
   * requestId exists, returns it. After confirm/fail/reap, a new slot is allocated.
   * @returns The reserved NonceSlot (mutable reference — use `getSlots()` for snapshot copies)
   * @throws If no free slots are available (pool exhausted)
   * @throws If pool is not initialized
   */
  acquire(requestId?: string): Readonly<NonceSlot> {
    this._assertInitialized();

    if (requestId !== undefined) {
      for (const s of this.slots.values()) {
        if (s.requestId === requestId && (s.state === "reserved" || s.state === "submitted")) {
          return Object.freeze({ ...s });
        }
      }
    }

    this.reap();

    let free: NonceSlot | undefined;
    for (const s of this.slots.values()) {
      if (s.state === "free") {
        free = s;
        break;
      }
    }
    if (!free) {
      throw new Error("NoncePool: no free slots available");
    }

    const now = Date.now();
    free.state = "reserved";
    free.reservedAt = now;
    free.requestId = requestId;

    if (this.mode === "expiring") {
      free.validBefore = Math.floor(now / 1000) + this.validBeforeOffsetS;
    }

    return Object.freeze({ ...free });
  }

  /**
   * Mark a reserved slot as submitted (transaction sent to mempool).
   *
   * @param nonceKey - The lane key of the slot
   * @param txHash - Transaction hash
   * @throws If no slot exists for `nonceKey`
   * @throws If slot is not in "reserved" state
   */
  submit(nonceKey: bigint, txHash: `0x${string}`): void {
    const slot = this._getSlot(nonceKey);
    if (slot.state !== "reserved") {
      throw new Error(
        `Cannot submit slot ${nonceKey}: state is "${slot.state}", expected "reserved"`,
      );
    }
    slot.state = "submitted";
    slot.submittedAt = Date.now();
    slot.txHash = txHash;
  }

  /**
   * Mark a submitted slot as confirmed (transaction included in a block).
   *
   * Increments the nonce and resets the slot to "free", making it available
   * for the next transaction. In expiring mode, callers needing a fresh
   * on-chain nonce should call `reset()` before the next `acquire()`.
   *
   * @param nonceKey - The lane key of the slot
   * @throws If no slot exists for `nonceKey`
   * @throws If slot is not in "submitted" state
   */
  confirm(nonceKey: bigint): void {
    const slot = this._getSlot(nonceKey);
    if (slot.state !== "submitted") {
      throw new Error(
        `Cannot confirm slot ${nonceKey}: state is "${slot.state}", expected "submitted"`,
      );
    }
    if (this.mode === "lanes") {
      slot.nonce += 1n;
    }
    this.confirmedCount++;
    this._resetSlot(slot);
  }

  /**
   * Mark a slot as failed (transaction rejected by the network).
   *
   * Resets the slot to "free" with the same nonce value, allowing the caller
   * to retry with the same nonce (the nonce was not consumed on-chain).
   *
   * Also accepts `reserved` state (cancels a reservation that was never
   * submitted). Unlike `release()`, this increments `failedCount` in stats.
   *
   * @param nonceKey - The lane key of the slot
   * @throws If no slot exists for `nonceKey`
   * @throws If slot is not in "submitted" or "reserved" state
   */
  fail(nonceKey: bigint): void {
    const slot = this._getSlot(nonceKey);
    if (slot.state !== "submitted" && slot.state !== "reserved") {
      throw new Error(
        `Cannot fail slot ${nonceKey}: state is "${slot.state}", expected "submitted" or "reserved"`,
      );
    }
    this.failedCount++;
    this._resetSlot(slot);
  }

  /**
   * Release a slot back to "free" state regardless of current state.
   * Use for explicit cleanup or cancellation.
   *
   * @param nonceKey - The lane key of the slot
   * @throws If no slot exists for `nonceKey`
   */
  release(nonceKey: bigint): void {
    const slot = this._getSlot(nonceKey);
    this._resetSlot(slot);
  }

  /**
   * Reclaim slots that have exceeded the reservation TTL.
   *
   * @returns Snapshots of slots as they were just before being reaped
   */
  reap(): NonceSlot[] {
    this._assertInitialized();
    const now = Date.now();
    const reaped: NonceSlot[] = [];

    for (const slot of this.slots.values()) {
      if (
        slot.state === "reserved" &&
        slot.reservedAt !== undefined &&
        now - slot.reservedAt > this.reservationTtlMs
      ) {
        reaped.push({ ...slot });
        this._resetSlot(slot);
      }
    }

    this.reapedCount += reaped.length;
    return reaped;
  }

  /** Get a snapshot of all slots (shallow copies — safe to read, won't affect internal state). */
  getSlots(): readonly Readonly<NonceSlot>[] {
    return [...this.slots.values()].map((s) => ({ ...s }));
  }

  /** Get aggregate statistics. */
  getStats(): NoncePoolStats {
    let free = 0;
    let reserved = 0;
    let submitted = 0;
    for (const s of this.slots.values()) {
      if (s.state === "free") free++;
      else if (s.state === "reserved") reserved++;
      else if (s.state === "submitted") submitted++;
    }
    return {
      total: this.slots.size,
      free,
      reserved,
      submitted,
      confirmed: this.confirmedCount,
      failed: this.failedCount,
      expired: this.reapedCount,
    };
  }

  /**
   * Re-query all on-chain nonce values and reset all slots to "free".
   * Cumulative stats (confirmed/failed/expired) are preserved.
   * Call after a node restart or detected chain reorganization.
   *
   */
  async reset(): Promise<void> {
    this._assertInitialized();
    this.slots = this._slotsToMap(await this._fetchInitialSlots());
  }

  private _slotsToMap(slots: NonceSlot[]): Map<bigint, NonceSlot> {
    return new Map(slots.map((s) => [s.nonceKey, s]));
  }

  private async _fetchInitialSlots(): Promise<NonceSlot[]> {
    const slots: NonceSlot[] = [];

    if (this.mode === "lanes") {
      const promises: Promise<bigint>[] = [];
      for (let i = 1; i <= this.laneCount; i++) {
        promises.push(getNonceFromPrecompile(this.client!, this.address, BigInt(i)));
      }
      const nonces = await Promise.all(promises);
      for (let i = 0; i < this.laneCount; i++) {
        slots.push({
          nonceKey: BigInt(i + 1),
          nonce: nonces[i]!,
          state: "free",
        });
      }
    } else {
      const nonce = await getNonceFromPrecompile(this.client!, this.address, MAX_UINT256);
      slots.push({
        nonceKey: MAX_UINT256,
        nonce,
        state: "free",
      });
    }

    return slots;
  }

  private _getSlot(nonceKey: bigint): NonceSlot {
    const slot = this.slots.get(nonceKey);
    if (!slot) {
      throw new Error(`NoncePool: slot not found for nonceKey=${nonceKey}`);
    }
    return slot;
  }

  private _resetSlot(slot: NonceSlot): void {
    slot.state = "free";
    slot.reservedAt = undefined;
    slot.submittedAt = undefined;
    slot.txHash = undefined;
    slot.requestId = undefined;
    slot.validBefore = undefined;
  }

  private _assertInitialized(): void {
    if (!this.initialized) {
      throw new Error("NoncePool not initialized — call init() first");
    }
  }
}
