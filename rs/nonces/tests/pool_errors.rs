use alloy::primitives::Address;
use tempo_reconcile_nonces::constants::{
    DEFAULT_LANES, DEFAULT_RESERVATION_TTL_MS, DEFAULT_VALID_BEFORE_OFFSET_S, MODERATO_CHAIN_ID,
};
use tempo_reconcile_nonces::types::{NonceMode, NoncePoolOptions};
use tempo_reconcile_nonces::NoncePool;

fn valid_options() -> NoncePoolOptions {
    NoncePoolOptions {
        address: Address::repeat_byte(0x01),
        rpc_url: "http://localhost:8545".to_string(),
        lanes: DEFAULT_LANES,
        mode: NonceMode::Lanes,
        reservation_ttl_ms: DEFAULT_RESERVATION_TTL_MS,
        valid_before_offset_s: DEFAULT_VALID_BEFORE_OFFSET_S,
        chain_id: MODERATO_CHAIN_ID,
        validate_chain_id: false,
    }
}

#[test]
fn rejects_zero_address() {
    let mut opts = valid_options();
    opts.address = Address::ZERO;
    let err = NoncePool::new(opts).unwrap_err();
    assert!(err.to_string().contains("address is required"));
}

#[test]
fn rejects_empty_rpc_url() {
    let mut opts = valid_options();
    opts.rpc_url = String::new();
    let err = NoncePool::new(opts).unwrap_err();
    assert!(err.to_string().contains("rpc_url is required"));
}

#[test]
fn rejects_zero_reservation_ttl() {
    let mut opts = valid_options();
    opts.reservation_ttl_ms = 0;
    let err = NoncePool::new(opts).unwrap_err();
    assert!(err.to_string().contains("reservation_ttl_ms must be > 0"));
}

#[test]
fn rejects_zero_valid_before_offset() {
    let mut opts = valid_options();
    opts.valid_before_offset_s = 0;
    let err = NoncePool::new(opts).unwrap_err();
    assert!(err
        .to_string()
        .contains("valid_before_offset_s must be > 0"));
}

#[test]
fn rejects_zero_lanes() {
    let mut opts = valid_options();
    opts.lanes = 0;
    let err = NoncePool::new(opts).unwrap_err();
    assert!(err.to_string().contains("lanes"));
}

#[test]
fn accepts_valid_options() {
    let pool = NoncePool::new(valid_options());
    assert!(pool.is_ok());
}

#[test]
fn chain_id_accessor() {
    let pool = NoncePool::new(valid_options()).unwrap();
    assert_eq!(pool.chain_id(), MODERATO_CHAIN_ID);
}

#[test]
fn acquire_before_init_fails() {
    let mut pool = NoncePool::new(valid_options()).unwrap();
    let err = pool.acquire(None).unwrap_err();
    assert!(err.to_string().contains("not initialized"));
}

#[tokio::test]
async fn double_init_fails() {
    // We can't truly double-init without RPC, but we can test
    // the already-initialized path via the test constructor
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 2, 30_000, 30);
    // pool is already initialized via new_for_testing
    let err = pool.init().await.unwrap_err();
    assert!(err.to_string().contains("already initialized"));
}

#[test]
fn submit_nonexistent_key_fails() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 30_000, 30);
    let err = pool
        .submit(
            alloy::primitives::U256::from(999),
            alloy::primitives::FixedBytes::from([0; 32]),
        )
        .unwrap_err();
    assert!(err.to_string().contains("slot not found"));
}

#[test]
fn confirm_nonexistent_key_fails() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 30_000, 30);
    let err = pool
        .confirm(alloy::primitives::U256::from(999))
        .unwrap_err();
    assert!(err.to_string().contains("slot not found"));
}

#[test]
fn fail_nonexistent_key_fails() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 30_000, 30);
    let err = pool.fail(alloy::primitives::U256::from(999)).unwrap_err();
    assert!(err.to_string().contains("slot not found"));
}

#[test]
fn release_nonexistent_key_fails() {
    let mut pool = NoncePool::new_for_testing(NonceMode::Lanes, 1, 30_000, 30);
    let err = pool
        .release(alloy::primitives::U256::from(999))
        .unwrap_err();
    assert!(err.to_string().contains("slot not found"));
}
