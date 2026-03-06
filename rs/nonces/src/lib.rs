//! Nonce pool for Tempo's 2D nonce system (TIP-1009).
//!
//! Supports two concurrency modes:
//! - **Lanes**: N independent parallel nonce sequences for high-throughput batching
//! - **Expiring**: Single TIP-1009 expiring nonce with time-bounded validity

pub mod constants;
pub mod error;
pub mod pool;
pub mod rpc;
pub mod types;

pub use constants::*;
pub use error::NonceError;
pub use pool::NoncePool;
pub use rpc::{get_nonce_from_precompile, get_protocol_nonce};
pub use types::*;
