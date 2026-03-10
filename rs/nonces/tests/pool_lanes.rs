use alloy_primitives::{FixedBytes, U256};
use tempo_reconcile_nonces::types::{NonceMode, SlotState};
use tempo_reconcile_nonces::NoncePool;

fn make_pool(lanes: u32) -> NoncePool {
    NoncePool::new_for_testing(NonceMode::Lanes, lanes, 30_000, 30)
}

#[test]
fn creates_correct_number_of_slots() {
    let pool = make_pool(4);
    assert_eq!(pool.slots().len(), 4);
    for slot in pool.slots() {
        assert_eq!(slot.state, SlotState::Free);
        assert_eq!(slot.nonce, 0);
    }
}

#[test]
fn slots_have_sequential_nonce_keys() {
    let pool = make_pool(4);
    for (i, slot) in pool.slots().iter().enumerate() {
        assert_eq!(slot.nonce_key, U256::from(i + 1));
    }
}

#[test]
fn acquire_returns_first_free_slot() {
    let mut pool = make_pool(4);
    let slot = pool.acquire(None).unwrap();
    assert_eq!(slot.state, SlotState::Reserved);
    assert_eq!(slot.nonce_key, U256::from(1));
    assert!(slot.reserved_at.is_some());
    assert!(slot.valid_before.is_none()); // lanes mode: no validBefore
}

#[test]
fn acquire_cycles_through_lanes() {
    let mut pool = make_pool(4);
    let k1 = pool.acquire(None).unwrap().nonce_key;
    let k2 = pool.acquire(None).unwrap().nonce_key;
    let k3 = pool.acquire(None).unwrap().nonce_key;
    let k4 = pool.acquire(None).unwrap().nonce_key;
    assert_eq!(k1, U256::from(1));
    assert_eq!(k2, U256::from(2));
    assert_eq!(k3, U256::from(3));
    assert_eq!(k4, U256::from(4));
}

#[test]
fn acquire_exhausted_when_all_reserved() {
    let mut pool = make_pool(2);
    pool.acquire(None).unwrap();
    pool.acquire(None).unwrap();
    let err = pool.acquire(None).unwrap_err();
    assert!(err.to_string().contains("no free slots"));
}

#[test]
fn submit_transitions_reserved_to_submitted() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    let tx_hash = FixedBytes::from([0xAB; 32]);
    pool.submit(key, tx_hash).unwrap();
    let slot = &pool.slots()[0];
    assert_eq!(slot.state, SlotState::Submitted);
    assert_eq!(slot.tx_hash, Some(tx_hash));
    assert!(slot.submitted_at.is_some());
}

#[test]
fn submit_fails_on_free_slot() {
    let mut pool = make_pool(1);
    let key = U256::from(1);
    let err = pool.submit(key, FixedBytes::from([0; 32])).unwrap_err();
    assert!(err.to_string().contains("state is \"free\""));
}

#[test]
fn confirm_increments_nonce_and_frees_slot() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0xAA; 32])).unwrap();
    pool.confirm(key).unwrap();

    let slot = &pool.slots()[0];
    assert_eq!(slot.state, SlotState::Free);
    assert_eq!(slot.nonce, 1);
    assert!(slot.tx_hash.is_none());
    assert!(slot.reserved_at.is_none());
    assert!(slot.submitted_at.is_none());
    assert!(slot.request_id.is_none());
}

#[test]
fn confirm_fails_on_reserved_slot() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    let err = pool.confirm(key).unwrap_err();
    assert!(err.to_string().contains("state is \"reserved\""));
}

#[test]
fn fail_from_reserved_preserves_nonce() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.fail(key).unwrap();

    let slot = &pool.slots()[0];
    assert_eq!(slot.state, SlotState::Free);
    assert_eq!(slot.nonce, 0); // unchanged
}

#[test]
fn fail_from_submitted_preserves_nonce() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0; 32])).unwrap();
    pool.fail(key).unwrap();

    let slot = &pool.slots()[0];
    assert_eq!(slot.state, SlotState::Free);
    assert_eq!(slot.nonce, 0);
}

#[test]
fn fail_on_free_slot_errors() {
    let mut pool = make_pool(1);
    let err = pool.fail(U256::from(1)).unwrap_err();
    assert!(err.to_string().contains("state is \"free\""));
}

#[test]
fn release_resets_any_state_to_free() {
    let mut pool = make_pool(1);
    let key = pool.acquire(None).unwrap().nonce_key;
    pool.submit(key, FixedBytes::from([0; 32])).unwrap();
    pool.release(key).unwrap();
    assert_eq!(pool.slots()[0].state, SlotState::Free);
}

#[test]
fn release_free_slot_is_noop() {
    let mut pool = make_pool(1);
    pool.release(U256::from(1)).unwrap();
    assert_eq!(pool.slots()[0].state, SlotState::Free);
}

#[test]
fn full_lifecycle_acquire_submit_confirm() {
    let mut pool = make_pool(2);

    // Lane 1: full cycle
    let k1 = pool.acquire(None).unwrap().nonce_key;
    pool.submit(k1, FixedBytes::from([0x11; 32])).unwrap();
    pool.confirm(k1).unwrap();
    assert_eq!(pool.slots()[0].nonce, 1);

    // Lane 1 is free again, can acquire
    let k = pool.acquire(None).unwrap().nonce_key;
    assert_eq!(k, k1); // reuses same lane
}

#[test]
fn slot_not_found_for_unknown_key() {
    let mut pool = make_pool(1);
    let err = pool
        .submit(U256::from(999), FixedBytes::from([0; 32]))
        .unwrap_err();
    assert!(err.to_string().contains("slot not found"));
}
