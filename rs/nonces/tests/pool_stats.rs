use alloy::primitives::FixedBytes;
use tempo_reconcile_nonces::types::NonceMode;
use tempo_reconcile_nonces::NoncePool;

fn make_pool() -> NoncePool {
    NoncePool::new_for_testing(NonceMode::Lanes, 4, 30_000, 30)
}

#[test]
fn initial_stats() {
    let pool = make_pool();
    let s = pool.stats();
    assert_eq!(s.total, 4);
    assert_eq!(s.free, 4);
    assert_eq!(s.reserved, 0);
    assert_eq!(s.submitted, 0);
    assert_eq!(s.confirmed, 0);
    assert_eq!(s.failed, 0);
    assert_eq!(s.expired, 0);
}

#[test]
fn stats_reflect_state_transitions() {
    let mut pool = make_pool();

    pool.acquire(None).unwrap();
    let s = pool.stats();
    assert_eq!(s.free, 3);
    assert_eq!(s.reserved, 1);

    let key = pool.slots()[0].nonce_key;
    pool.submit(key, FixedBytes::from([0; 32])).unwrap();
    let s = pool.stats();
    assert_eq!(s.reserved, 0);
    assert_eq!(s.submitted, 1);

    pool.confirm(key).unwrap();
    let s = pool.stats();
    assert_eq!(s.free, 4);
    assert_eq!(s.submitted, 0);
    assert_eq!(s.confirmed, 1);
}

#[test]
fn cumulative_counters_accumulate() {
    let mut pool = make_pool();

    for _ in 0..3 {
        let key = pool.acquire(None).unwrap().nonce_key;
        pool.submit(key, FixedBytes::from([0; 32])).unwrap();
        pool.confirm(key).unwrap();
    }

    let key = pool.acquire(None).unwrap().nonce_key;
    pool.fail(key).unwrap();

    let s = pool.stats();
    assert_eq!(s.confirmed, 3);
    assert_eq!(s.failed, 1);
}

#[test]
fn release_does_not_increment_failed_or_reaped_count() {
    let mut pool = make_pool();
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.release(key).unwrap();

    let s = pool.stats();
    assert_eq!(s.failed, 0, "release must not increment failed_count");
    assert_eq!(s.expired, 0, "release must not increment reaped_count");
    assert_eq!(s.free, 4, "slot must return to free after release");
}

#[test]
fn stats_total_always_consistent() {
    let mut pool = make_pool();
    pool.acquire(None).unwrap();
    pool.acquire(None).unwrap();
    let key = pool.slots()[0].nonce_key;
    pool.submit(key, FixedBytes::from([0; 32])).unwrap();

    let s = pool.stats();
    assert_eq!(s.total, s.free + s.reserved + s.submitted);
}
