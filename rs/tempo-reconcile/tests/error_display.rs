#![cfg(all(
    feature = "watcher",
    feature = "watcher-ws",
    feature = "explorer",
    feature = "webhook"
))]

use tempo_reconcile::{ExplorerError, MemoError, ReconcileError, WatcherError, WebhookError};

// ── MemoError ─────────────────────────────────────────────────────────────

#[test]
fn memo_error_invalid_ulid_display() {
    let e = MemoError::InvalidUlid("bad-ulid".to_string());
    assert_eq!(e.to_string(), "invalid ULID: bad-ulid");
}

#[test]
fn memo_error_invalid_ulid_empty_display() {
    let e = MemoError::InvalidUlid(String::new());
    assert_eq!(e.to_string(), "invalid ULID: ");
}

// ── ReconcileError ────────────────────────────────────────────────────────

#[test]
fn reconcile_error_duplicate_expected_display() {
    let e = ReconcileError::DuplicateExpected(
        "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
    );
    assert_eq!(
        e.to_string(),
        "duplicate expected payment for memo_raw: \
         0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
    );
}

#[test]
fn reconcile_error_duplicate_expected_empty_display() {
    let e = ReconcileError::DuplicateExpected(String::new());
    assert_eq!(e.to_string(), "duplicate expected payment for memo_raw: ");
}

// ── WatcherError ─────────────────────────────────────────────────────────

#[test]
fn watcher_error_rpc_display() {
    let e = WatcherError::Rpc("connection refused".to_string());
    assert_eq!(e.to_string(), "RPC error: connection refused");
}

#[test]
fn watcher_error_http_display() {
    let e = WatcherError::Http("404 Not Found".to_string());
    assert_eq!(e.to_string(), "HTTP error: 404 Not Found");
}

#[test]
fn watcher_error_ws_display() {
    let e = WatcherError::Ws("connection closed".to_string());
    assert_eq!(e.to_string(), "WebSocket error: connection closed");
}

// ── ExplorerError ─────────────────────────────────────────────────────────

#[test]
fn explorer_error_http_display() {
    let e = ExplorerError::Http("503 Service Unavailable".to_string());
    assert_eq!(e.to_string(), "HTTP error: 503 Service Unavailable");
}

#[test]
fn explorer_error_not_found_display() {
    let e = ExplorerError::NotFound("0xabc123".to_string());
    assert_eq!(e.to_string(), "not found: 0xabc123");
}

#[test]
fn explorer_error_parse_display() {
    let e = ExplorerError::Parse("unexpected field 'foo'".to_string());
    assert_eq!(e.to_string(), "parse error: unexpected field 'foo'");
}

// ── WebhookError ──────────────────────────────────────────────────────────

#[test]
fn webhook_error_http_display() {
    let e = WebhookError::Http("401 Unauthorized".to_string());
    assert_eq!(e.to_string(), "HTTP error: 401 Unauthorized");
}
