//! # tempo-reconcile
//!
//! Reconciliation library for TIP-20 payments on Tempo.
//! Implements the [TEMPO-RECONCILE-MEMO-001](https://github.com/valpaq/tempo-reconciliation-sdk/blob/main/spec/MEMO-SPEC.md)
//! bytes32 memo standard.
//!
//! ## Modules
//!
//! - **memo** — pure functions: encode/decode bytes32, issuer tag, ULID conversion
//! - **reconciler** — stateful matching engine with pluggable [`ReconcileStore`]
//! - **export** — CSV, JSON, JSONL formatters
//! - **watcher** — `eth_getLogs` fetcher and polling watcher (`feature = "watcher"`)
//! - **webhook** — HMAC-signed HTTP delivery (`feature = "webhook"`)
//!
//! ## Quick start
//!
//! ```rust
//! use tempo_reconcile::{
//!     issuer_tag_from_namespace, encode_memo_v1, EncodeMemoV1Params,
//!     Reconciler, ReconcilerOptions, ExpectedPayment, MemoType,
//! };
//!
//! fn run() -> Result<(), Box<dyn std::error::Error>> {
//!     let tag = issuer_tag_from_namespace("my-app");
//!     let memo = encode_memo_v1(&EncodeMemoV1Params {
//!         memo_type: MemoType::Invoice,
//!         issuer_tag: tag,
//!         ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
//!         salt: None,
//!     })?;
//!
//!     let mut reconciler = Reconciler::new(ReconcilerOptions::new());
//!     reconciler.expect(ExpectedPayment {
//!         memo_raw: memo,
//!         token: "0x20c0000000000000000000000000000000000000".to_string(),
//!         to: "0xrecipient".to_string(),
//!         amount: 10_000_000,
//!         from: None,
//!         due_at: None,
//!         meta: None,
//!     })?;
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod export;
pub mod memo;
pub mod reconciler;
pub mod types;

#[cfg(feature = "watcher")]
pub mod watcher;

#[cfg(feature = "explorer")]
pub mod explorer;

pub use error::{MemoError, ReconcileError};

#[cfg(feature = "watcher")]
pub use error::WatcherError;

#[cfg(feature = "webhook")]
pub use error::WebhookError;

#[cfg(feature = "explorer")]
pub use error::ExplorerError;

// memo
pub use memo::decode::{decode_memo, decode_memo_text, decode_memo_v1, is_memo_v1};
pub use memo::encode::{encode_memo_v1, EncodeMemoV1Params};
pub use memo::issuer_tag::issuer_tag_from_namespace;
pub use memo::ulid::{bytes16_to_ulid, ulid_to_bytes16};

#[cfg(feature = "rand")]
pub use memo::encode::random_salt;

// reconciler
pub use reconciler::engine::{Reconciler, ReconcilerOptions, ToleranceMode};
pub use reconciler::store::{InMemoryStore, ReconcileStore};

// export
#[cfg(feature = "export")]
pub use export::csv::export_csv;
#[cfg(feature = "export")]
pub use export::json::{export_json, export_jsonl};

#[cfg(feature = "webhook")]
pub use export::webhook::{send_webhook, sign, WebhookBatchError, WebhookConfig, WebhookResult};

// watcher
#[cfg(feature = "watcher")]
pub use watcher::{get_tip20_transfer_history, watch_tip20_transfers, WatchConfig, WatchHandle};

#[cfg(feature = "watcher-ws")]
pub use watcher::{watch_tip20_transfers_ws, WatchWsConfig};

// explorer
#[cfg(feature = "explorer")]
pub use explorer::{
    create_explorer_client, AddressMetadata, BalancesResponse, ExplorerClient, ExplorerTransaction,
    HistoryResponse, KnownEvent, KnownEventPart, KnownEventPartValue, TokenBalance,
};

// types
pub use types::{
    ExpectedPayment, MatchResult, MatchStatus, Memo, MemoType, MemoV1, PaymentEvent,
    ReconcileReport, ReconcileSummary,
};
