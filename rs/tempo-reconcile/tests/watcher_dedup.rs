#![cfg(feature = "watcher")]
use std::time::Duration;
use tempo_reconcile::watcher::DedupCache;

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

#[test]
fn re_insert_after_ttl_expiry_returns_true() {
    let mut cache = DedupCache::new(0, 100); // TTL=0: expires immediately
    assert!(cache.check_and_insert("key1"));
    // After TTL expires, the same key should be accepted as new
    std::thread::sleep(Duration::from_millis(10));
    assert!(
        cache.check_and_insert("key1"),
        "re-inserting after TTL expiry must return true"
    );
}
