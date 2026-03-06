/**
 * Time-windowed deduplication cache keyed on `(txHash, logIndex)`.
 * TTL is measured from insertion time; reads do not extend the TTL.
 */
export class DedupCache {
  private cache = new Map<string, number>();
  private ttlMs: number;
  private maxSize: number;

  constructor(ttlMs = 60_000, maxSize = 10_000) {
    this.ttlMs = ttlMs;
    this.maxSize = maxSize;
  }

  has(txHash: string, logIndex: number): boolean {
    const key = `${txHash.toLowerCase()}:${logIndex}`;
    const ts = this.cache.get(key);
    if (ts === undefined) return false;
    return Date.now() - ts <= this.ttlMs;
  }

  add(txHash: string, logIndex: number): void {
    const key = `${txHash.toLowerCase()}:${logIndex}`;
    this.cache.set(key, Date.now());
    this.evict();
  }

  private collectExpired(): string[] {
    const now = Date.now();
    const expired: string[] = [];
    for (const [key, ts] of this.cache) {
      if (now - ts > this.ttlMs) expired.push(key);
    }
    return expired;
  }

  private evict(): void {
    if (this.cache.size <= this.maxSize) return;

    for (const key of this.collectExpired()) this.cache.delete(key);

    if (this.cache.size > this.maxSize) {
      const excess = this.cache.size - this.maxSize;
      const oldest: string[] = [];
      for (const key of this.cache.keys()) {
        if (oldest.length >= excess) break;
        oldest.push(key);
      }
      for (const key of oldest) this.cache.delete(key);
    }
  }

  clear(): void {
    this.cache.clear();
  }

  /** Entry count including expired entries not yet evicted. */
  get size(): number {
    return this.cache.size;
  }

  /**
   * Remove all entries whose TTL has elapsed.
   *
   * @returns Number of entries removed
   */
  prune(): number {
    const expired = this.collectExpired();
    for (const key of expired) this.cache.delete(key);
    return expired.length;
  }
}
