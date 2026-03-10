use alloy_primitives::{FixedBytes, U256};
use std::time::Instant;

/// Nonce pool operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceMode {
    /// Independent parallel lanes (nonceKey 1..N). High throughput.
    Lanes,
    /// Single TIP-1009 expiring nonce (nonceKey = MAX_U256). Time-bounded.
    Expiring,
}

/// Slot lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotState {
    Free,
    Reserved,
    Submitted,
}

impl SlotState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotState::Free => "free",
            SlotState::Reserved => "reserved",
            SlotState::Submitted => "submitted",
        }
    }
}

/// A single nonce slot managed by the pool.
#[derive(Debug, Clone)]
pub struct NonceSlot {
    pub nonce_key: U256,
    pub nonce: u64,
    pub state: SlotState,
    pub reserved_at: Option<Instant>,
    pub submitted_at: Option<Instant>,
    pub tx_hash: Option<FixedBytes<32>>,
    pub request_id: Option<String>,
    /// Unix timestamp in seconds (expiring mode only).
    pub valid_before: Option<u64>,
}

impl NonceSlot {
    pub(crate) fn new(nonce_key: U256, nonce: u64) -> Self {
        Self {
            nonce_key,
            nonce,
            state: SlotState::Free,
            reserved_at: None,
            submitted_at: None,
            tx_hash: None,
            request_id: None,
            valid_before: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.state = SlotState::Free;
        self.reserved_at = None;
        self.submitted_at = None;
        self.tx_hash = None;
        self.request_id = None;
        self.valid_before = None;
    }
}

/// Configuration for creating a [`NoncePool`](crate::NoncePool).
#[derive(Debug, Clone)]
pub struct NoncePoolOptions {
    /// Sender account address (required).
    pub address: alloy_primitives::Address,
    /// RPC endpoint URL (required).
    pub rpc_url: String,
    /// Number of parallel lanes (default: 4). Only used in Lanes mode.
    pub lanes: u32,
    /// Operating mode (default: Lanes).
    pub mode: NonceMode,
    /// Reservation auto-expiry in milliseconds (default: 30_000).
    pub reservation_ttl_ms: u64,
    /// Offset for TIP-1009 validBefore in seconds (default: 30).
    pub valid_before_offset_s: u64,
    /// Chain ID (default: 42431 = Moderato).
    pub chain_id: u64,
    /// Whether to validate chain ID against RPC at init (default: false).
    pub validate_chain_id: bool,
}

impl Default for NoncePoolOptions {
    fn default() -> Self {
        Self {
            address: alloy_primitives::Address::ZERO,
            rpc_url: String::new(),
            lanes: crate::constants::DEFAULT_LANES,
            mode: NonceMode::Lanes,
            reservation_ttl_ms: crate::constants::DEFAULT_RESERVATION_TTL_MS,
            valid_before_offset_s: crate::constants::DEFAULT_VALID_BEFORE_OFFSET_S,
            chain_id: crate::constants::MODERATO_CHAIN_ID,
            validate_chain_id: false,
        }
    }
}

/// Aggregate statistics for a nonce pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoncePoolStats {
    /// Total managed slots.
    pub total: usize,
    /// Available slots.
    pub free: usize,
    /// Reserved but not submitted.
    pub reserved: usize,
    /// Pending on-chain confirmation.
    pub submitted: usize,
    /// Cumulative confirmed count.
    pub confirmed: u64,
    /// Cumulative failed count.
    pub failed: u64,
    /// Cumulative reaped/expired count.
    pub expired: u64,
}
