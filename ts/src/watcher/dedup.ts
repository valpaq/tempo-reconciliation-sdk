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
    if (Date.now() - ts > this.ttlMs) {
      this.cache.delete(key);
      return false;
    }
    return true;
  }

  add(txHash: string, logIndex: number): void {
    const key = `${txHash.toLowerCase()}:${logIndex}`;
    this.cache.set(key, Date.now());
    this.evict();
  }

  private evict(): void {
    if (this.cache.size <= this.maxSize) return;
    const now = Date.now();
    for (const [key, ts] of this.cache) {
      if (now - ts > this.ttlMs) {
        this.cache.delete(key);
      }
    }

    if (this.cache.size > this.maxSize) {
      const entries = [...this.cache.entries()];
      entries.sort((a, b) => a[1] - b[1]);
      const toRemove = entries.slice(0, entries.length - this.maxSize);
      for (const [key] of toRemove) {
        this.cache.delete(key);
      }
    }
  }

  clear(): void {
    this.cache.clear();
  }

  get size(): number {
    return this.cache.size;
  }
}
