use alloy::primitives::U256;

/// Errors produced by the nonce pool.
#[derive(Debug, thiserror::Error)]
pub enum NonceError {
    #[error("NoncePool: address is required")]
    MissingAddress,

    #[error("NoncePool: rpc_url is required")]
    MissingRpcUrl,

    #[error("NoncePool: lanes must be >= 1")]
    InvalidLanes,

    #[error("NoncePool: reservation_ttl_ms must be > 0")]
    InvalidTtl,

    #[error("NoncePool: valid_before_offset_s must be > 0")]
    InvalidValidBefore,

    #[error("NoncePool: not initialized — call init() first")]
    NotInitialized,

    #[error("NoncePool: already initialized — call reset() to re-sync nonces")]
    AlreadyInitialized,

    #[error("NoncePool: no free slots available")]
    Exhausted,

    #[error("NoncePool: slot not found for nonce_key={0}")]
    SlotNotFound(U256),

    #[error("Cannot {action} slot {nonce_key}: state is \"{actual}\", expected \"{expected}\"")]
    InvalidState {
        action: &'static str,
        nonce_key: U256,
        actual: &'static str,
        expected: &'static str,
    },

    #[error("NoncePool: chainId mismatch — configured {configured}, RPC returned {actual}")]
    ChainIdMismatch { configured: u64, actual: u64 },

    #[error("NoncePool: rpc error: {0}")]
    Rpc(#[from] alloy::contract::Error),
}
