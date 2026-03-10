import { describe, it, expect, vi, afterEach } from "vitest";
import { DedupCache } from "../../src/watcher/dedup";

describe("DedupCache", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("returns false for unseen entry", () => {
    const cache = new DedupCache();
    expect(cache.has("0xabc", 0)).toBe(false);
  });

  it("returns true after adding entry", () => {
    const cache = new DedupCache();
    cache.add("0xabc", 0);
    expect(cache.has("0xabc", 0)).toBe(true);
  });

  it("matches case-insensitively on txHash", () => {
    const cache = new DedupCache();
    cache.add("0xABC123def456789000000000000000000000000000000000000000000000abcd", 0);
    expect(cache.has("0xabc123def456789000000000000000000000000000000000000000000000abcd", 0)).toBe(
      true,
    );
  });

  it("differentiates by logIndex", () => {
    const cache = new DedupCache();
    cache.add("0xabc", 0);
    expect(cache.has("0xabc", 1)).toBe(false);
  });

  it("differentiates by txHash", () => {
    const cache = new DedupCache();
    cache.add("0xabc", 0);
    expect(cache.has("0xdef", 0)).toBe(false);
  });

  it("expires entries after TTL", () => {
    const cache = new DedupCache(100);
    cache.add("0xabc", 0);

    // advance time past TTL
    vi.spyOn(Date, "now").mockReturnValueOnce(Date.now() + 200);
    expect(cache.has("0xabc", 0)).toBe(false);
  });

  it("tracks size", () => {
    const cache = new DedupCache();
    expect(cache.size).toBe(0);
    cache.add("0xa", 0);
    cache.add("0xb", 1);
    expect(cache.size).toBe(2);
  });

  it("clears all entries", () => {
    const cache = new DedupCache();
    cache.add("0xa", 0);
    cache.add("0xb", 1);
    cache.clear();
    expect(cache.size).toBe(0);
    expect(cache.has("0xa", 0)).toBe(false);
  });

  it("does not extend TTL on has() check", () => {
    const cache = new DedupCache(100);
    const baseTime = Date.now();
    vi.spyOn(Date, "now").mockReturnValue(baseTime);

    cache.add("0xabc", 0);

    // At 50% of TTL, check the entry (should not extend)
    vi.spyOn(Date, "now").mockReturnValue(baseTime + 60);
    expect(cache.has("0xabc", 0)).toBe(true);

    // At 110ms (past original TTL of 100ms), entry should be expired
    vi.spyOn(Date, "now").mockReturnValue(baseTime + 110);
    expect(cache.has("0xabc", 0)).toBe(false);
  });

  it("evicts when over maxSize", () => {
    const cache = new DedupCache(60_000, 3);
    cache.add("0xa", 0);
    cache.add("0xb", 0);
    cache.add("0xc", 0);
    cache.add("0xd", 0); // triggers evict, should drop oldest
    expect(cache.size).toBeLessThanOrEqual(3);
  });

  it("evicts oldest entries first by insertion order", () => {
    const cache = new DedupCache(60_000, 2);
    cache.add("0xa", 0);
    cache.add("0xb", 0);
    cache.add("0xc", 0); // triggers evict, "0xa" should be dropped

    expect(cache.has("0xa", 0)).toBe(false);
    expect(cache.has("0xb", 0)).toBe(true);
    expect(cache.has("0xc", 0)).toBe(true);
  });
});

describe("DedupCache.prune", () => {
  it("removes expired entries and returns count", () => {
    const cache = new DedupCache(100);
    const baseTime = Date.now();
    vi.spyOn(Date, "now").mockReturnValue(baseTime);

    cache.add("0xaaa", 0);
    cache.add("0xbbb", 0);

    // Advance time past TTL
    vi.spyOn(Date, "now").mockReturnValue(baseTime + 200);
    const removed = cache.prune();
    expect(removed).toBe(2);
    expect(cache.size).toBe(0);
  });

  it("leaves non-expired entries intact", () => {
    const cache = new DedupCache(1000);
    const baseTime = Date.now();
    vi.spyOn(Date, "now").mockReturnValue(baseTime);

    cache.add("0xaaa", 0);

    // Not expired yet
    vi.spyOn(Date, "now").mockReturnValue(baseTime + 500);
    const removed = cache.prune();
    expect(removed).toBe(0);
    expect(cache.has("0xaaa", 0)).toBe(true);
  });

  it("returns 0 when cache is empty", () => {
    const cache = new DedupCache();
    expect(cache.prune()).toBe(0);
  });
});
