/**
 * Time-windowed deduplication cache keyed on `(txHash, logIndex)`.
 *
 * TTL is measured from insertion time; reads do not extend the TTL.
 * When the cache exceeds `maxSize`, expired entries are evicted first,
 * then the oldest entries are dropped.
 */
export class DedupCache {
  private cache = new Map<string, number>();
  private ttlMs: number;
  private maxSize: number;

  /**
   * @param ttlMs - Time-to-live in milliseconds for each entry (default 60 000)
   * @param maxSize - Maximum number of entries before eviction (default 10 000)
   */
  constructor(ttlMs = 60_000, maxSize = 10_000) {
    this.ttlMs = ttlMs;
    this.maxSize = maxSize;
  }

  /**
   * Check whether a `(txHash, logIndex)` pair exists and has not expired.
   *
   * @param txHash - Transaction hash (case-insensitive)
   * @param logIndex - Log index within the transaction
   * @returns `true` if the entry exists and its TTL has not elapsed
   */
  has(txHash: string, logIndex: number): boolean {
    const key = `${txHash.toLowerCase()}:${logIndex}`;
    const ts = this.cache.get(key);
    if (ts === undefined) return false;
    return Date.now() - ts <= this.ttlMs;
  }

  /**
   * Insert or update a `(txHash, logIndex)` entry with the current timestamp.
   * Triggers eviction if the cache exceeds `maxSize`.
   *
   * @param txHash - Transaction hash (case-insensitive)
   * @param logIndex - Log index within the transaction
   */
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
