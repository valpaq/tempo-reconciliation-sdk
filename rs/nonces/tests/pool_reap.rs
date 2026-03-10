use tempo_reconcile_nonces::types::{NonceMode, SlotState};
use tempo_reconcile_nonces::NoncePool;

#[test]
fn reap_returns_empty_when_no_stale_slots() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 2, 30_000, 30);
    pool.acquire(None).unwrap();
    let reaped = pool.reap();
    assert!(reaped.is_empty());
}

#[test]
fn reap_reclaims_stale_reservations() {
    // Use 1ms TTL so reservations expire immediately
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 2, 1, 30);
    pool.acquire(None).unwrap();
    pool.acquire(None).unwrap();

    // Wait for TTL to expire
    std::thread::sleep(std::time::Duration::from_millis(5));

    let reaped = pool.reap();
    assert_eq!(reaped.len(), 2);
    assert_eq!(reaped[0].state, SlotState::Reserved); // snapshot before reset

    // Slots are free now
    for slot in pool.slots() {
        assert_eq!(slot.state, SlotState::Free);
    }
}

#[test]
fn reap_preserves_nonce() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 1, 30);
    pool.acquire(None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    pool.reap();
    assert_eq!(pool.slots()[0].nonce, 0); // nonce unchanged
}

#[test]
fn reap_does_not_touch_submitted_slots() {
    use alloy_primitives::FixedBytes;
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 1, 30);
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0; 32])).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(5));
    let reaped = pool.reap();
    assert!(reaped.is_empty()); // submitted slots not reaped
    assert_eq!(pool.slots()[0].state, SlotState::Submitted);
}

#[test]
fn auto_reap_on_acquire_when_exhausted() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 1, 30);
    pool.acquire(None).unwrap();

    // Pool is exhausted (1 slot, 1 reserved)
    std::thread::sleep(std::time::Duration::from_millis(5));

    // This triggers auto-reap, then acquires the freed slot
    let slot = pool.acquire(None).unwrap();
    assert_eq!(slot.state, SlotState::Reserved);
}

#[test]
fn reap_expiring_mode_reserved_past_ttl() {
    // Create expiring mode pool with 1ms TTL so reservations expire immediately
    let mut pool = NoncePool::new_for_testing(NonceMode::Expiring, 1, 1, 30);
    pool.acquire(None).unwrap();

    // Wait for TTL to expire
    std::thread::sleep(std::time::Duration::from_millis(5));

    let reaped = pool.reap();
    assert_eq!(reaped.len(), 1);
    assert_eq!(reaped[0].state, SlotState::Reserved); // snapshot before reset

    // Slot is free now — can acquire again
    let slot = pool.acquire(None).unwrap();
    assert_eq!(slot.state, SlotState::Reserved);
}

#[test]
fn reaped_count_tracks_cumulative() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 2, 1, 30);
    pool.acquire(None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    pool.reap();

    pool.acquire(None).unwrap();
    pool.acquire(None).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    pool.reap();

    assert_eq!(pool.stats().expired, 3); // 1 + 2
}
