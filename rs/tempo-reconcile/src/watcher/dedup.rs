use std::collections::HashMap;
use std::time::{Duration, Instant};

/// TTL-based deduplication cache keyed by `"{tx_hash}:{log_index}"`.
///
/// Eviction is lazy: expired entries are removed only when the cache reaches `max_size`.
pub struct DedupCache {
    inner: HashMap<String, Instant>,
    ttl: Duration,
    max_size: usize,
}

impl DedupCache {
    pub fn new(ttl_secs: u64, max_size: usize) -> Self {
        Self {
            inner: HashMap::new(),
            ttl: Duration::from_secs(ttl_secs),
            max_size: max_size.max(1),
        }
    }

    /// Returns `true` and marks `key` as seen if it has not been seen within the TTL window.
    /// Returns `false` (duplicate) otherwise.
    pub fn check_and_insert(&mut self, key: &str) -> bool {
        let now = Instant::now();

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
