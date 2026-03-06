/// Errors returned by memo encoding functions.
#[derive(Debug, thiserror::Error)]
pub enum MemoError {
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
}

/// Errors returned by [`crate::Reconciler`].
#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error("duplicate expected payment for memo_raw: {0}")]
    DuplicateExpected(String),
    #[error("amount_tolerance_bps must be <= 10000, got {0}")]
    InvalidToleranceBps(u32),
}

/// Errors returned by watcher functions.
#[cfg(feature = "watcher")]
#[derive(Debug, thiserror::Error)]
pub enum WatcherError {
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[cfg(feature = "watcher-ws")]
    #[error("WebSocket error: {0}")]
    Ws(String),
}

/// Errors returned by [`crate::explorer::ExplorerClient`].
#[cfg(feature = "explorer")]
#[derive(Debug, thiserror::Error)]
pub enum ExplorerError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("parse error: {0}")]
    Parse(String),
}

/// Errors returned by [`crate::send_webhook`].
#[cfg(feature = "webhook")]
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("HTTP error: {0}")]
    Http(String),
}
