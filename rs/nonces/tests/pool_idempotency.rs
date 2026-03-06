use alloy::primitives::FixedBytes;
use tempo_reconcile_nonces::types::{NonceMode, SlotState};
use tempo_reconcile_nonces::NoncePool;

fn make_pool() -> NoncePool {
    NoncePool::new_for_testing(NonceMode::Lanes, 4, 30_000, 30)
}

#[test]
fn acquire_with_request_id_returns_same_slot() {
    let mut pool = make_pool();
    let slot1 = pool.acquire(Some("pay-001")).unwrap();
    let key1 = slot1.nonce_key;

    let slot2 = pool.acquire(Some("pay-001")).unwrap();
    let key2 = slot2.nonce_key;

    assert_eq!(key1, key2); // same slot returned
}

#[test]
fn acquire_different_request_ids_get_different_slots() {
    let mut pool = make_pool();
    let k1 = pool.acquire(Some("pay-001")).unwrap().nonce_key;
    let k2 = pool.acquire(Some("pay-002")).unwrap().nonce_key;
    assert_ne!(k1, k2);
}

#[test]
fn idempotent_acquire_in_submitted_state() {
    let mut pool = make_pool();
    let key = pool.acquire(Some("pay-001")).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0xAA; 32])).unwrap();

    // Still returns same slot even after submit
    let slot = pool.acquire(Some("pay-001")).unwrap();
    assert_eq!(slot.nonce_key, key);
    assert_eq!(slot.state, SlotState::Submitted);
}

#[test]
fn after_confirm_same_request_id_gets_new_slot() {
    let mut pool = make_pool();
    let key = pool.acquire(Some("pay-001")).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0xAA; 32])).unwrap();
    pool.confirm(key).unwrap();

    // requestId cleared after confirm, new slot allocated
    let slot = pool.acquire(Some("pay-001")).unwrap();
    // Could be same lane (it's free now) but it's a fresh allocation
    assert_eq!(slot.state, SlotState::Reserved);
}

#[test]
fn after_fail_same_request_id_gets_new_slot() {
    let mut pool = make_pool();
    let key = pool.acquire(Some("pay-001")).unwrap().nonce_key;
    pool.fail(key).unwrap();

    let slot = pool.acquire(Some("pay-001")).unwrap();
    assert_eq!(slot.state, SlotState::Reserved);
}

#[test]
fn acquire_idempotent_expiring_mode() {
    // Expiring mode has a single slot (MAX_U256 key)
    let mut pool = NoncePool::new_for_testing(NonceMode::Expiring, 1, 30_000, 30);
    let slot1 = pool.acquire(Some("exp-pay-001")).unwrap();
    let key1 = slot1.nonce_key;

    // Same request_id — must return the same slot
    let slot2 = pool.acquire(Some("exp-pay-001")).unwrap();
    let key2 = slot2.nonce_key;

    assert_eq!(key1, key2);
    assert_eq!(slot2.state, SlotState::Reserved);
}

#[test]
fn no_request_id_always_allocates_new() {
    let mut pool = make_pool();
    let k1 = pool.acquire(None).unwrap().nonce_key;
    let k2 = pool.acquire(None).unwrap().nonce_key;
    assert_ne!(k1, k2);
}
