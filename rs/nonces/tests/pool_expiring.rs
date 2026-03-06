use alloy::primitives::FixedBytes;
use tempo_reconcile_nonces::constants::MAX_U256;
use tempo_reconcile_nonces::types::{NonceMode, SlotState};
use tempo_reconcile_nonces::NoncePool;

fn make_pool() -> NoncePool {
    NoncePool::new_for_testing(NonceMode::Expiring, 1, 30_000, 30)
}

#[test]
fn creates_single_slot_with_max_u256_key() {
    let pool = make_pool();
    assert_eq!(pool.slots().len(), 1);
    assert_eq!(pool.slots()[0].nonce_key, MAX_U256);
}

#[test]
fn acquire_sets_valid_before() {
    let mut pool = make_pool();
    let slot = pool.acquire(None).unwrap();
    assert!(slot.valid_before.is_some());
    let vb = slot.valid_before.unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // validBefore should be approximately now + 30s
    assert!(vb >= now + 28 && vb <= now + 32);
}

#[test]
fn single_slot_exhaustion() {
    let mut pool = make_pool();
    pool.acquire(None).unwrap();
    let err = pool.acquire(None).unwrap_err();
    assert!(err.to_string().contains("no free slots"));
}

#[test]
fn confirm_frees_slot_for_reuse() {
    let mut pool = make_pool();
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0xCC; 32])).unwrap();
    pool.confirm(key).unwrap();

    assert_eq!(pool.slots()[0].nonce, 1);
    assert_eq!(pool.slots()[0].state, SlotState::Free);
    assert!(pool.slots()[0].valid_before.is_none()); // cleared on confirm

    // Can acquire again
    let slot = pool.acquire(None).unwrap();
    assert!(slot.valid_before.is_some()); // new validBefore set
    assert_eq!(slot.nonce, 1); // incremented from previous confirm
}

#[test]
fn fail_clears_valid_before() {
    let mut pool = make_pool();
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.fail(key).unwrap();
    assert!(pool.slots()[0].valid_before.is_none());
    assert_eq!(pool.slots()[0].nonce, 0); // unchanged
}

#[test]
fn custom_valid_before_offset() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Expiring, 1, 30_000, 60);
    let slot = pool.acquire(None).unwrap();
    let vb = slot.valid_before.unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(vb >= now + 58 && vb <= now + 62);
}

#[test]
fn full_lifecycle_expiring() {
    let mut pool = make_pool();

    // Cycle 1
    let key = pool.acquire(None).unwrap().nonce_key;
    assert_eq!(key, MAX_U256);
    pool.submit(key, FixedBytes::from([0x11; 32])).unwrap();
    pool.confirm(key).unwrap();

    // Cycle 2
    let slot = pool.acquire(None).unwrap();
    assert_eq!(slot.nonce, 1);
    pool.submit(key, FixedBytes::from([0x22; 32])).unwrap();
    pool.confirm(key).unwrap();
    assert_eq!(pool.slots()[0].nonce, 2);
}
