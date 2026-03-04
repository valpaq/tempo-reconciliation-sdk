use std::collections::HashMap;
use std::time::{Duration, Instant};

/// TTL-based deduplication cache keyed by `"{tx_hash}:{log_index}"`.
///
/// Eviction is lazy: expired entries are removed only when the cache reaches `max_size`.
pub(super) struct DedupCache {
    inner: HashMap<String, Instant>,
    ttl: Duration,
    max_size: usize,
}

impl DedupCache {
    pub(super) fn new(ttl_secs: u64, max_size: usize) -> Self {
        Self {
            inner: HashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
            max_size: max_size.max(1),
        }
    }

    /// Returns `true` and marks `key` as seen if it has not been seen within the TTL window.
    /// Returns `false` (duplicate) otherwise.
    pub(super) fn check_and_insert(&mut self, key: &str) -> bool {
        let now = Instant::now();

        // Dedup check first: if the key is already fresh, return false immediately
        // without touching the eviction logic.
        if let Some(ts) = self.inner.get(key) {
            if now.duration_since(*ts) < self.ttl {
                return false;
            }
        }

        // Key is new or expired. Evict to make room if at capacity.
        if self.inner.len() >= self.max_size {
            // Phase 1: evict expired entries
            self.inner
                .retain(|_, ts| now.duration_since(*ts) < self.ttl);

            // Phase 2: if still at capacity, evict oldest (LRU)
            if self.inner.len() >= self.max_size {
                if let Some(oldest_key) = self
                    .inner
                    .iter()
                    .min_by_key(|(_, ts)| *ts)
                    .map(|(k, _)| k.clone())
                {
                    self.inner.remove(&oldest_key);
                }
            }
        }

        self.inner.insert(key.to_string(), now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_key_returns_true() {
        let mut cache = DedupCache::new(60, 100);
        assert!(cache.check_and_insert("0xabc:0"));
    }

    #[test]
    fn duplicate_key_returns_false() {
        let mut cache = DedupCache::new(60, 100);
        assert!(cache.check_and_insert("0xabc:0"));
        assert!(!cache.check_and_insert("0xabc:0"));
    }

    #[test]
    fn max_size_evicts_expired() {
        let mut cache = DedupCache::new(0, 2); // TTL=0 means everything expires instantly
        cache.check_and_insert("a");
        cache.check_and_insert("b");
        // Both should be expired (TTL=0). Inserting "c" should trigger eviction.
        // Sleep briefly to ensure the TTL has truly elapsed:
        std::thread::sleep(Duration::from_millis(10));
        assert!(cache.check_and_insert("c")); // triggers eviction, inserts new
                                              // "a" should be evicted (expired), re-inserting should return true
        assert!(cache.check_and_insert("a"));
    }

    #[test]
    fn max_size_zero_clamped_to_one() {
        let mut cache = DedupCache::new(60, 0); // max_size=0, clamped to 1
        assert!(cache.check_and_insert("first"));
        // Cache is at max (1), but "first" hasn't expired, so it's deduped
        assert!(!cache.check_and_insert("first"));
    }

    #[test]
    fn lru_evicts_oldest_when_all_fresh() {
        let mut cache = DedupCache::new(3600, 2); // long TTL, max 2
        cache.check_and_insert("a");
        cache.check_and_insert("b");
        // At capacity, all fresh. "a" is oldest → evicted.
        std::thread::sleep(Duration::from_millis(5));
        cache.check_and_insert("c"); // evicts "a"
        assert!(cache.check_and_insert("a")); // "a" was evicted, new again
        assert!(!cache.check_and_insert("c")); // "c" still in cache
    }
}
