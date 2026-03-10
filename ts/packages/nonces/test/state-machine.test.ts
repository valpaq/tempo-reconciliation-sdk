import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createPool } from "./helpers";

vi.mock("../src/rpc", () => ({
  getNonceFromPrecompile: vi.fn().mockResolvedValue(0n),
  getProtocolNonce: vi.fn().mockResolvedValue(0n),
}));

vi.mock("viem", () => ({
  createPublicClient: vi.fn().mockReturnValue({}),
  http: vi.fn(),
  isAddress: vi.fn().mockReturnValue(true),
}));

describe("lanes mode full lifecycle", () => {
  it("cycles through acquire → submit → confirm across multiple lanes", async () => {
    const pool = await createPool({ lanes: 2 });

    // Lane 1: full cycle
    const s1 = pool.acquire("tx-1");
    expect(s1.nonceKey).toBe(1n);
    expect(s1.nonce).toBe(0n);
    pool.submit(s1.nonceKey, "0x1111");
    pool.confirm(s1.nonceKey);

    // Lane 1 recycled with nonce 1
    const s3 = pool.acquire("tx-3");
    expect(s3.nonceKey).toBe(1n);
    expect(s3.nonce).toBe(1n);

    // Lane 2: full cycle
    const s2 = pool.acquire("tx-2");
    expect(s2.nonceKey).toBe(2n);
    pool.submit(s2.nonceKey, "0x2222");
    pool.confirm(s2.nonceKey);
  });

  it("retries with same nonce after fail", async () => {
    const pool = await createPool({ lanes: 1 });
    const s1 = pool.acquire();
    const nonce = s1.nonce;

    pool.submit(s1.nonceKey, "0xfail");
    pool.fail(s1.nonceKey);

    // Same lane, same nonce — ready for retry
    const s2 = pool.acquire();
    expect(s2.nonceKey).toBe(1n);
    expect(s2.nonce).toBe(nonce);
  });

  it("nonce increments correctly over multiple confirm cycles", async () => {
    const pool = await createPool({ lanes: 1 });

    for (let i = 0; i < 5; i++) {
      const slot = pool.acquire();
      expect(slot.nonce).toBe(BigInt(i));
      pool.submit(slot.nonceKey, `0x${i.toString(16)}` as `0x${string}`);
      pool.confirm(slot.nonceKey);
    }
  });

  it("handles mixed confirm and fail across lanes", async () => {
    const pool = await createPool({ lanes: 3 });

    const s1 = pool.acquire();
    const s2 = pool.acquire();
    const s3 = pool.acquire();

    pool.submit(s1.nonceKey, "0x01");
    pool.submit(s2.nonceKey, "0x02");
    pool.submit(s3.nonceKey, "0x03");

    pool.confirm(s1.nonceKey); // lane 1: nonce 0 → 1
    pool.fail(s2.nonceKey); // lane 2: nonce stays 0
    pool.confirm(s3.nonceKey); // lane 3: nonce 0 → 1

    const stats = pool.getStats();
    expect(stats.free).toBe(3);

    // Verify nonce values
    const slots = pool.getSlots();
    expect(slots[0]!.nonce).toBe(1n); // lane 1: confirmed
    expect(slots[1]!.nonce).toBe(0n); // lane 2: failed, same nonce
    expect(slots[2]!.nonce).toBe(1n); // lane 3: confirmed
  });
});

describe("expiring mode full lifecycle", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("acquire sets validBefore, confirm resets to free", async () => {
    const pool = await createPool({ mode: "expiring", validBeforeOffsetS: 30 });

    const slot = pool.acquire("exp-1");
    expect(slot.validBefore).toBeDefined();
    expect(slot.state).toBe("reserved");

    pool.submit(slot.nonceKey, "0xexp");
    const afterSubmit = pool.getSlots().find((s) => s.nonceKey === slot.nonceKey)!;
    expect(afterSubmit.state).toBe("submitted");

    pool.confirm(slot.nonceKey);
    // Confirm always recycles to free (no terminal states)
    const afterConfirm = pool.getSlots().find((s) => s.nonceKey === slot.nonceKey)!;
    expect(afterConfirm.state).toBe("free");
    expect(afterConfirm.validBefore).toBeUndefined();
    expect(afterConfirm.nonce).toBe(0n);
  });

  it("fail resets to free with same nonce for retry", async () => {
    const pool = await createPool({ mode: "expiring" });

    const slot = pool.acquire();
    const nonce = slot.nonce;
    pool.submit(slot.nonceKey, "0xfail");
    pool.fail(slot.nonceKey);
    const current = pool.getSlots().find((s) => s.nonceKey === slot.nonceKey)!;
    expect(current.state).toBe("free");
    expect(current.nonce).toBe(nonce);
  });

  it("reap resets expired reservation to free", async () => {
    const pool = await createPool({ mode: "expiring", reservationTtlMs: 1 });
    pool.acquire();

    vi.advanceTimersByTime(5);

    const reaped = pool.reap();
    expect(reaped).toHaveLength(1);
    expect(pool.getSlots()[0]!.state).toBe("free");

    // Pool is usable again without reset()
    const slot = pool.acquire();
    expect(slot.state).toBe("reserved");
  });
});

describe("idempotency edge cases", () => {
  it("different requestIds get different slots", async () => {
    const pool = await createPool({ lanes: 4 });
    const s1 = pool.acquire("a");
    const s2 = pool.acquire("b");
    const s3 = pool.acquire("a"); // duplicate
    expect(s1.nonceKey).not.toBe(s2.nonceKey);
    // s3 is a frozen snapshot of the same slot as s1
    expect(s3.nonceKey).toBe(s1.nonceKey);
    expect(s3.requestId).toBe("a");
  });

  it("requestId undefined always allocates new slot", async () => {
    const pool = await createPool({ lanes: 4 });
    const s1 = pool.acquire();
    const s2 = pool.acquire();
    expect(s1.nonceKey).not.toBe(s2.nonceKey);
  });
});

describe("exhaustion and recovery", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("pool exhaustion recovers after confirm frees a slot", async () => {
    const pool = await createPool({ lanes: 1 });
    const slot = pool.acquire();

    expect(() => pool.acquire()).toThrow("no free slots available");

    pool.submit(slot.nonceKey, "0x01");
    pool.confirm(slot.nonceKey);

    // Now the slot is free again
    const s2 = pool.acquire();
    expect(s2.state).toBe("reserved");
    expect(s2.nonce).toBe(1n);
  });

  it("pool exhaustion recovers after release", async () => {
    const pool = await createPool({ lanes: 1 });
    pool.acquire();

    expect(() => pool.acquire()).toThrow("no free slots available");

    pool.release(1n);
    const s2 = pool.acquire();
    expect(s2.state).toBe("reserved");
  });

  it("pool exhaustion recovers via auto-reap on stale reservations", async () => {
    const pool = await createPool({ lanes: 1, reservationTtlMs: 1 });
    pool.acquire();

    vi.advanceTimersByTime(5);

    // acquire() calls reap() internally, freeing the stale slot
    const s2 = pool.acquire();
    expect(s2.state).toBe("reserved");
  });
});
