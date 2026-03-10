use alloy_primitives::{address, Address, U256};

/// INonce precompile address on Tempo (ASCII "NONCE" zero-padded to 20 bytes).
pub const NONCE_PRECOMPILE: Address = address!("4e4F4E4345000000000000000000000000000000");

/// `2^256 - 1` — the nonce key used in expiring (TIP-1009) mode.
pub const MAX_U256: U256 = U256::MAX;

/// Moderato testnet chain ID.
pub const MODERATO_CHAIN_ID: u64 = 42431;

/// Default number of parallel lanes.
pub const DEFAULT_LANES: u32 = 4;

/// Default reservation TTL in milliseconds.
pub const DEFAULT_RESERVATION_TTL_MS: u64 = 30_000;

/// Default validBefore offset in seconds.
pub const DEFAULT_VALID_BEFORE_OFFSET_S: u64 = 30;
