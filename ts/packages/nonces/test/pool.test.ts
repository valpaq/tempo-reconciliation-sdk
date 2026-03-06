import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { NoncePool } from "../src/pool";
import { createPublicClient, isAddress } from "viem";
import { defaultOpts, createPool } from "./helpers";

// Mock the rpc module so we don't make real RPC calls
vi.mock("../src/rpc", () => ({
  getNonceFromPrecompile: vi.fn().mockResolvedValue(0n),
  getProtocolNonce: vi.fn().mockResolvedValue(0n),
}));

// Mock viem so createPublicClient doesn't try to connect
vi.mock("viem", () => ({
  createPublicClient: vi.fn().mockReturnValue({}),
  http: vi.fn(),
  isAddress: vi.fn().mockReturnValue(true),
}));

describe("NoncePool constructor", () => {
  it("throws if address is invalid", () => {
    vi.mocked(isAddress).mockReturnValueOnce(false);
    expect(() => new NoncePool({ address: "0xBAD" as `0x${string}`, rpcUrl: "http://x" })).toThrow(
      "NoncePool: invalid address format",
    );
  });

  it("throws if lanes > 1 in expiring mode", () => {
    expect(() => new NoncePool({ ...defaultOpts, mode: "expiring", lanes: 5 })).toThrow(
      "NoncePool: lanes option is not supported in expiring mode",
    );
  });

  it("throws if rpcUrl is empty", () => {
    expect(() => new NoncePool({ address: "0x01", rpcUrl: "" })).toThrow("NoncePool: rpcUrl is required");
  });

  it("throws if lanes < 1", () => {
    expect(() => new NoncePool({ ...defaultOpts, lanes: 0 })).toThrow("NoncePool: lanes must be an integer >= 1");
  });

  it("throws if lanes is NaN", () => {
    expect(() => new NoncePool({ ...defaultOpts, lanes: NaN })).toThrow("NoncePool: lanes must be an integer >= 1");
  });

  it("throws if lanes is non-integer", () => {
    expect(() => new NoncePool({ ...defaultOpts, lanes: 1.5 })).toThrow("NoncePool: lanes must be an integer >= 1");
  });

  it("throws if reservationTtlMs <= 0", () => {
    expect(() => new NoncePool({ ...defaultOpts, reservationTtlMs: 0 })).toThrow(
      "NoncePool: reservationTtlMs must be > 0",
    );
  });

  it("throws if validBeforeOffsetS is negative", () => {
    expect(() => new NoncePool({ ...defaultOpts, validBeforeOffsetS: -10 })).toThrow(
      "NoncePool: validBeforeOffsetS must be > 0",
    );
  });

  it("throws if validBeforeOffsetS is zero", () => {
    expect(() => new NoncePool({ ...defaultOpts, validBeforeOffsetS: 0 })).toThrow(
      "NoncePool: validBeforeOffsetS must be > 0",
    );
  });

  it("uses default values", () => {
    const pool = new NoncePool(defaultOpts);
    expect(pool.chainId).toBe(42431);
  });

  it("accepts custom chainId", () => {
    const pool = new NoncePool({ ...defaultOpts, chainId: 4217 });
    expect(pool.chainId).toBe(4217);
  });
});

describe("NoncePool.init", () => {
  it("creates slots for each lane", async () => {
    const pool = await createPool({ lanes: 3 });
    const slots = pool.getSlots();
    expect(slots).toHaveLength(3);
    expect(slots[0]!.nonceKey).toBe(1n);
    expect(slots[1]!.nonceKey).toBe(2n);
    expect(slots[2]!.nonceKey).toBe(3n);
  });

  it("creates single slot in expiring mode", async () => {
    const pool = await createPool({ mode: "expiring" });
    const slots = pool.getSlots();
    expect(slots).toHaveLength(1);
    expect(slots[0]!.nonceKey).toBe(2n ** 256n - 1n);
  });

  it("all slots start as free", async () => {
    const pool = await createPool({ lanes: 4 });
    for (const slot of pool.getSlots()) {
      expect(slot.state).toBe("free");
    }
  });

  it("initializes slots with non-zero nonce from RPC", async () => {
    const { getNonceFromPrecompile } = await import("../src/rpc");
    vi.mocked(getNonceFromPrecompile).mockResolvedValueOnce(5n);
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();
    expect(slot.nonce).toBe(5n);
    vi.mocked(getNonceFromPrecompile).mockResolvedValue(0n);
  });
});

describe("NoncePool.acquire", () => {
  it("returns the first free slot", async () => {
    const pool = await createPool({ lanes: 2 });
    const slot = pool.acquire();
    expect(slot.nonceKey).toBe(1n);
    expect(slot.state).toBe("reserved");
  });

  it("marks slot as reserved with timestamp", async () => {
    const pool = await createPool();
    const before = Date.now();
    const slot = pool.acquire();
    expect(slot.state).toBe("reserved");
    expect(slot.reservedAt).toBeGreaterThanOrEqual(before);
  });

  it("assigns requestId to slot", async () => {
    const pool = await createPool();
    const slot = pool.acquire("payment-123");
    expect(slot.requestId).toBe("payment-123");
  });

  it("returns different slots for consecutive acquires", async () => {
    const pool = await createPool({ lanes: 3 });
    const s1 = pool.acquire();
    const s2 = pool.acquire();
    expect(s1.nonceKey).not.toBe(s2.nonceKey);
  });

  it("throws when pool is exhausted", async () => {
    const pool = await createPool({ lanes: 1 });
    pool.acquire();
    expect(() => pool.acquire()).toThrow("NoncePool: no free slots available");
  });

  it("throws if not initialized", () => {
    const pool = new NoncePool(defaultOpts);
    expect(() => pool.acquire()).toThrow("not initialized");
  });

  it("returns existing slot for duplicate requestId (idempotency)", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire("req-1");
    const s2 = pool.acquire("req-1");
    expect(s1).toBe(s2);
    expect(s1.nonceKey).toBe(s2.nonceKey);
  });

  it("does not return confirmed slot for same requestId", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire("req-1");
    pool.submit(s1.nonceKey, "0xaaa");
    pool.confirm(s1.nonceKey);
    // s1 is now free (recycled), requestId cleared
    const s2 = pool.acquire("req-1");
    // Should get a new reservation, not the old one
    expect(s2.state).toBe("reserved");
  });

  it("sets validBefore in expiring mode", async () => {
    const pool = await createPool({ mode: "expiring", validBeforeOffsetS: 30 });
    const nowSec = Math.floor(Date.now() / 1000);
    const slot = pool.acquire();
    expect(slot.validBefore).toBeGreaterThanOrEqual(nowSec + 29);
    expect(slot.validBefore).toBeLessThanOrEqual(nowSec + 31);
  });

  it("does not set validBefore in lanes mode", async () => {
    const pool = await createPool({ mode: "lanes" });
    const slot = pool.acquire();
    expect(slot.validBefore).toBeUndefined();
  });
});

describe("NoncePool.submit", () => {
  it("transitions reserved → submitted", async () => {
    const pool = await createPool();
    const slot = pool.acquire();
    pool.submit(slot.nonceKey, "0xdeadbeef");
    expect(slot.state).toBe("submitted");
    expect(slot.txHash).toBe("0xdeadbeef");
    expect(slot.submittedAt).toBeDefined();
  });

  it("throws if slot is not reserved", async () => {
    const pool = await createPool();
    expect(() => pool.submit(1n, "0xabc")).toThrow('state is "free", expected "reserved"');
  });

  it("throws if slot not found", async () => {
    const pool = await createPool({ lanes: 1 });
    expect(() => pool.submit(99n, "0xabc")).toThrow("slot not found");
  });
});

describe("NoncePool.getSlots", () => {
  it("returns snapshot copies — mutations do not affect pool state", async () => {
    const pool = await createPool({ lanes: 1 });
    const slots = pool.getSlots();
    // Mutate the returned snapshot
    (slots[0] as { state: string }).state = "submitted";
    // Pool's internal slot must be unaffected
    expect(pool.getSlots()[0]!.state).toBe("free");
  });

  it("reflects current state in each call", async () => {
    const pool = await createPool({ lanes: 1 });
    expect(pool.getSlots()[0]!.state).toBe("free");
    pool.acquire();
    expect(pool.getSlots()[0]!.state).toBe("reserved");
  });
});

describe("NoncePool.confirm", () => {
  it("transitions submitted → confirmed and recycles in lanes mode", async () => {
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    pool.submit(slot.nonceKey, "0xaaa");
    pool.confirm(slot.nonceKey);

    // In lanes mode: slot is recycled to free with incremented nonce
    const slots = pool.getSlots();
    expect(slots[0]!.state).toBe("free");
    expect(slots[0]!.nonce).toBe(originalNonce + 1n);
    expect(slots[0]!.txHash).toBeUndefined();
  });

  it("recycles to free in expiring mode without incrementing nonce", async () => {
    const pool = await createPool({ mode: "expiring" });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    pool.submit(slot.nonceKey, "0xaaa");
    pool.confirm(slot.nonceKey);
    expect(slot.state).toBe("free");
    expect(slot.nonce).toBe(originalNonce);
  });

  it("throws if slot is not submitted", async () => {
    const pool = await createPool();
    pool.acquire();
    expect(() => pool.confirm(1n)).toThrow('state is "reserved", expected "submitted"');
  });

  it("throws if slot not found", async () => {
    const pool = await createPool({ lanes: 1 });
    expect(() => pool.confirm(99n)).toThrow("slot not found");
  });
});

describe("NoncePool.fail", () => {
  it("transitions submitted → failed → free in lanes mode", async () => {
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    pool.submit(slot.nonceKey, "0xaaa");
    pool.fail(slot.nonceKey);

    // In lanes mode: slot is free with same nonce (for retry)
    expect(pool.getSlots()[0]!.state).toBe("free");
    expect(pool.getSlots()[0]!.nonce).toBe(originalNonce);
  });

  it("transitions to free in expiring mode (same nonce for retry)", async () => {
    const pool = await createPool({ mode: "expiring" });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    pool.fail(slot.nonceKey);
    expect(slot.state).toBe("free");
    expect(slot.nonce).toBe(originalNonce);
  });

  it("throws on invalid state", async () => {
    const pool = await createPool();
    expect(() => pool.fail(1n)).toThrow('state is "free"');
  });

  it("throws if slot not found", async () => {
    const pool = await createPool({ lanes: 1 });
    expect(() => pool.fail(99n)).toThrow("slot not found");
  });
});

describe("NoncePool.release", () => {
  it("resets any slot to free", async () => {
    const pool = await createPool();
    const slot = pool.acquire("req-1");
    pool.submit(slot.nonceKey, "0xaaa");
    pool.release(slot.nonceKey);
    expect(slot.state).toBe("free");
    expect(slot.txHash).toBeUndefined();
    expect(slot.requestId).toBeUndefined();
    expect(slot.reservedAt).toBeUndefined();
    expect(slot.submittedAt).toBeUndefined();
    expect(slot.validBefore).toBeUndefined();
  });

  it("resets expiring mode slot to free", async () => {
    const pool = await createPool({ mode: "expiring" });
    const slot = pool.acquire();
    expect(slot.validBefore).toBeDefined();
    pool.release(slot.nonceKey);
    expect(slot.state).toBe("free");
    expect(slot.validBefore).toBeUndefined();
  });

  it("does not increment confirmed or failed counters", async () => {
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();
    pool.release(slot.nonceKey);
    const stats = pool.getStats();
    expect(stats.confirmed).toBe(0);
    expect(stats.failed).toBe(0);
  });
});

describe("NoncePool.reap", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns empty array when no slots are expired", async () => {
    const pool = await createPool();
    pool.acquire();
    expect(pool.reap()).toEqual([]);
  });

  it("reclaims reserved slots past TTL", async () => {
    const pool = await createPool({ lanes: 1, reservationTtlMs: 1 });
    pool.acquire("req-stale");

    vi.advanceTimersByTime(5);

    const reaped = pool.reap();
    expect(reaped).toHaveLength(1);
    // Returned snapshots contain pre-reset state
    expect(reaped[0]!.state).toBe("reserved");
    expect(reaped[0]!.reservedAt).toBeDefined();
    expect(reaped[0]!.requestId).toBe("req-stale");
    // Actual slot is now free
    expect(pool.getSlots()[0]!.state).toBe("free");
  });

  it("does not reclaim submitted slots", async () => {
    const pool = await createPool({ lanes: 1, reservationTtlMs: 1 });
    const slot = pool.acquire();
    pool.submit(slot.nonceKey, "0xaaa");

    vi.advanceTimersByTime(5);

    const reaped = pool.reap();
    expect(reaped).toHaveLength(0);
    expect(pool.getSlots()[0]!.state).toBe("submitted");
  });

  it("acquire auto-reaps before declaring exhaustion", async () => {
    const pool = await createPool({ lanes: 1, reservationTtlMs: 1 });
    pool.acquire("old-req");

    vi.advanceTimersByTime(5);

    const slot = pool.acquire("new-req");
    expect(slot.state).toBe("reserved");
    expect(slot.requestId).toBe("new-req");
  });
});

describe("NoncePool.getStats", () => {
  it("counts slots by state correctly", async () => {
    const pool = await createPool({ lanes: 4 });
    pool.acquire();
    const s2 = pool.acquire();
    pool.submit(s2.nonceKey, "0xaaa");

    const stats = pool.getStats();
    expect(stats.total).toBe(4);
    expect(stats.free).toBe(2);
    expect(stats.reserved).toBe(1);
    expect(stats.submitted).toBe(1);
  });

  it("tracks cumulative confirmed/failed/expired counts", async () => {
    vi.useFakeTimers();
    try {
      const pool = await createPool({ lanes: 3, reservationTtlMs: 1 });

      // Confirm one
      const s1 = pool.acquire();
      pool.submit(s1.nonceKey, "0xaaa");
      pool.confirm(s1.nonceKey);

      // Fail one
      const s2 = pool.acquire();
      pool.submit(s2.nonceKey, "0xbbb");
      pool.fail(s2.nonceKey);

      // Reap one
      pool.acquire();
      vi.advanceTimersByTime(5);
      pool.reap();

      const stats = pool.getStats();
      expect(stats.confirmed).toBe(1);
      expect(stats.failed).toBe(1);
      expect(stats.expired).toBe(1);
      expect(stats.free).toBe(3); // all returned to free
    } finally {
      vi.useRealTimers();
    }
  });

  it("preserves cumulative counters after reset()", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire();
    pool.submit(s1.nonceKey, "0xaaa");
    pool.confirm(s1.nonceKey);

    expect(pool.getStats().confirmed).toBe(1);

    await pool.reset();

    // Stats accumulate across resets — reset() only re-syncs nonce values
    expect(pool.getStats().confirmed).toBe(1);
    expect(pool.getStats().failed).toBe(0);
    expect(pool.getStats().expired).toBe(0);
  });
});

describe("NoncePool.reset", () => {
  it("re-queries chain and resets all slots", async () => {
    const pool = await createPool({ lanes: 2 });
    pool.acquire();
    pool.acquire();

    await pool.reset();

    const stats = pool.getStats();
    expect(stats.free).toBe(2);
    expect(stats.reserved).toBe(0);
  });

  it("throws if not initialized", async () => {
    const pool = new NoncePool(defaultOpts);
    await expect(pool.reset()).rejects.toThrow("not initialized");
  });

  it("reset() throws when init() has not succeeded", async () => {
    const pool = new NoncePool({
      address: "0x1234567890abcdef1234567890abcdef12345678",
      rpcUrl: "http://localhost:8545",
      chainId: 42431,
    });
    // Pool was never initialized
    await expect(pool.reset()).rejects.toThrow("not initialized");
  });
});

describe("NoncePool.init validateChainId", () => {
  it("passes when chainId matches RPC", async () => {
    vi.mocked(createPublicClient).mockReturnValueOnce({
      getChainId: vi.fn().mockResolvedValue(42431),
    } as never);
    const pool = new NoncePool({ ...defaultOpts, chainId: 42431, validateChainId: true });
    await expect(pool.init()).resolves.toBeUndefined();
  });

  it("throws when chainId does not match RPC", async () => {
    vi.mocked(createPublicClient).mockReturnValueOnce({
      getChainId: vi.fn().mockResolvedValue(1),
    } as never);
    const pool = new NoncePool({ ...defaultOpts, chainId: 42431, validateChainId: true });
    await expect(pool.init()).rejects.toThrow(
      "NoncePool: chainId mismatch — configured 42431, RPC returned 1",
    );
  });

  it("skips RPC check when validateChainId is false (default)", async () => {
    // Default mock returns {} with no getChainId — would throw if called
    const pool = new NoncePool({ ...defaultOpts, chainId: 42431 });
    await expect(pool.init()).resolves.toBeUndefined();
  });
});

describe("NoncePool additional edge cases", () => {
  it("fail() transitions reserved → free (skips submitted)", async () => {
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    // fail without submitting first
    pool.fail(slot.nonceKey);
    expect(pool.getSlots()[0]!.state).toBe("free");
    expect(pool.getSlots()[0]!.nonce).toBe(originalNonce);
  });

  it("double init() throws after first call", async () => {
    const pool = await createPool();
    await expect(pool.init()).rejects.toThrow("NoncePool: already initialized");
  });

  it("concurrent init() calls — second rejects with already in progress", async () => {
    const pool = new NoncePool(defaultOpts);
    const [r1, r2] = await Promise.allSettled([pool.init(), pool.init()]);
    const results = [r1, r2];
    const fulfilled = results.filter((r) => r.status === "fulfilled");
    const rejected = results.filter((r) => r.status === "rejected");
    expect(fulfilled.length).toBe(1);
    expect(rejected.length).toBe(1);
    expect((rejected[0] as PromiseRejectedResult).reason.message).toMatch(
      /already in progress|already initialized/,
    );
  });

  it("acquire(requestId) returns submitted slot for same requestId", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire("req-inflight");
    pool.submit(s1.nonceKey, "0xaaa");
    expect(s1.state).toBe("submitted");
    // Same requestId — should return the submitted slot, not a new one
    const s2 = pool.acquire("req-inflight");
    expect(s2).toBe(s1);
    expect(s2.state).toBe("submitted");
  });

  it("init() RPC failure clears initializing flag so retry is possible", async () => {
    const { getNonceFromPrecompile } = await import("../src/rpc");
    vi.mocked(getNonceFromPrecompile).mockRejectedValueOnce(new Error("RPC timeout"));

    const pool = new NoncePool(defaultOpts);
    await expect(pool.init()).rejects.toThrow("RPC timeout");

    // After failure, can retry
    vi.mocked(getNonceFromPrecompile).mockResolvedValue(0n);
    await expect(pool.init()).resolves.toBeUndefined();
  });

  it("reset() with in-flight submitted slots resets all to free", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire();
    pool.submit(s1.nonceKey, "0xaaa");
    const s2 = pool.acquire();
    pool.submit(s2.nonceKey, "0xbbb");

    // Both slots are submitted — reset should clear all to free
    await pool.reset();

    const stats = pool.getStats();
    expect(stats.free).toBe(2);
    expect(stats.submitted).toBe(0);
  });

  it("release() on already-free slot is idempotent", async () => {
    const pool = await createPool({ lanes: 1 });
    // Slot 1 is free by default
    pool.release(1n);
    expect(pool.getSlots()[0]!.state).toBe("free");
  });

  it("reap() throws if not initialized", () => {
    const pool = new NoncePool(defaultOpts);
    expect(() => pool.reap()).toThrow("not initialized");
  });

  it("getStats() returns zeroes before init()", () => {
    const pool = new NoncePool(defaultOpts);
    const stats = pool.getStats();
    expect(stats.total).toBe(0);
    expect(stats.free).toBe(0);
  });

  it("confirm() does not increment nonce in expiring mode", async () => {
    const pool = await createPool({ mode: "expiring" });
    const slot = pool.acquire();
    const originalNonce = slot.nonce;
    pool.submit(slot.nonceKey, "0xaaa");
    pool.confirm(slot.nonceKey);
    expect(slot.state).toBe("free");
    expect(slot.nonce).toBe(originalNonce);
  });

  it("acquire() after fail() with same requestId allocates a fresh slot", async () => {
    const pool = await createPool({ lanes: 2 });
    const s1 = pool.acquire("retry-req");
    const nonceKey1 = s1.nonceKey;

    // fail() resets slot to free and clears requestId
    pool.fail(nonceKey1);
    expect(pool.getSlots().find((s) => s.nonceKey === nonceKey1)?.state).toBe("free");

    // Same requestId — since the previous slot is free (requestId cleared),
    // a new reservation is created
    const s2 = pool.acquire("retry-req");
    expect(s2.state).toBe("reserved");
    expect(s2.requestId).toBe("retry-req");
  });
});
