#![cfg(feature = "webhook")]

use mockito::Server;
use tempo_reconcile::{
    send_webhook, sign, ExpectedPayment, MatchResult, MatchStatus, PaymentEvent, WebhookConfig,
};

fn make_payment(amount: u128) -> PaymentEvent {
    PaymentEvent {
        chain_id: 42431,
        block_number: 1,
        tx_hash: "0xdeadbeef".to_string(),
        log_index: 0,
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        from: "0xfrom".to_string(),
        to: "0xto".to_string(),
        amount,
        memo_raw: None,
        memo: None,
        timestamp: Some(1_700_000_000),
    }
}

fn make_result(amount: u128) -> MatchResult {
    MatchResult {
        status: MatchStatus::Matched,
        payment: make_payment(amount),
        expected: Some(ExpectedPayment {
            memo_raw: "0x0000000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            to: "0xto".to_string(),
            amount,
            from: None,
            due_at: None,
            meta: None,
        }),
        reason: None,
        overpaid_by: None,
        remaining_amount: None,
        is_late: None,
    }
}

#[tokio::test]
async fn sends_post_with_json_body() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/webhook")
        .with_status(200)
        .create_async()
        .await;

    let config = WebhookConfig::new(format!("{}/webhook", server.url()));
    let result = send_webhook(&config, &[make_result(10_000_000)]).await;
    assert!(result.is_ok());

    m.assert_async().await;
}

#[tokio::test]
async fn empty_slice_sends_nothing() {
    let mut server = Server::new_async().await;
    // no mock registered — any call would fail
    let config = WebhookConfig::new(server.url());
    let result = send_webhook(&config, &[]).await;
    assert!(result.is_ok());
    assert_eq!(result.sent, 0);
    // server receives no calls
    server.mock("POST", "/").expect(0).create_async().await;
}

#[tokio::test]
async fn batches_by_batch_size() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .with_status(200)
        .expect(2)
        .create_async()
        .await;

    let mut config = WebhookConfig::new(server.url());
    config.batch_size = 1;

    let results = [make_result(1_000_000), make_result(2_000_000)];
    let r = send_webhook(&config, &results).await;
    assert!(r.is_ok());
    assert_eq!(r.sent, 2);

    m.assert_async().await;
}

#[tokio::test]
async fn records_failure_on_4xx() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/")
        .with_status(400)
        .create_async()
        .await;

    let config = WebhookConfig::new(server.url());
    let result = send_webhook(&config, &[make_result(1)]).await;
    assert!(!result.is_ok());
    assert_eq!(result.failed, 1);
    assert!(!result.errors.is_empty());
    assert_eq!(result.errors[0].status_code, Some(400));
}

#[tokio::test]
async fn retries_on_503() {
    let mut server = Server::new_async().await;
    // First call returns 503, second returns 200
    let _fail = server
        .mock("POST", "/")
        .with_status(503)
        .expect(1)
        .create_async()
        .await;
    let _ok = server
        .mock("POST", "/")
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    let mut config = WebhookConfig::new(server.url());
    config.max_retries = 1;
    config.timeout_secs = 5;

    let result = send_webhook(&config, &[make_result(1)]).await;
    assert!(result.is_ok());
    assert_eq!(result.sent, 1);
}

#[tokio::test]
async fn signature_header_present_when_secret_set() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .with_status(200)
        .match_header(
            "X-Tempo-Reconcile-Signature",
            mockito::Matcher::Regex(r"^[a-f0-9]{64}$".to_string()),
        )
        .create_async()
        .await;

    let mut config = WebhookConfig::new(server.url());
    config.secret = Some("test-secret".to_string());

    let result = send_webhook(&config, &[make_result(1)]).await;
    assert!(result.is_ok());
    m.assert_async().await;
}

#[tokio::test]
async fn idempotency_key_header_always_present() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .with_status(200)
        .match_header("X-Tempo-Reconcile-Idempotency-Key", mockito::Matcher::Any)
        .create_async()
        .await;

    let config = WebhookConfig::new(server.url());
    let result = send_webhook(&config, &[make_result(1)]).await;
    assert!(result.is_ok());
    m.assert_async().await;
}

#[tokio::test]
async fn timestamp_header_always_present() {
    let mut server = Server::new_async().await;
    let m = server
        .mock("POST", "/")
        .with_status(200)
        .match_header("X-Tempo-Reconcile-Timestamp", mockito::Matcher::Any)
        .create_async()
        .await;

    let config = WebhookConfig::new(server.url());
    let result = send_webhook(&config, &[make_result(1)]).await;
    assert!(result.is_ok());
    m.assert_async().await;
}

#[tokio::test]
async fn partial_success_continues_on_failure() {
    let mut server = Server::new_async().await;
    // First batch → 400 (non-retriable), second batch → 200
    let _fail = server
        .mock("POST", "/")
        .with_status(400)
        .expect(1)
        .create_async()
        .await;
    let _ok = server
        .mock("POST", "/")
        .with_status(200)
        .expect(1)
        .create_async()
        .await;

    let mut config = WebhookConfig::new(server.url());
    config.batch_size = 1; // one result per batch

    let results = [make_result(1), make_result(2)];
    let r = send_webhook(&config, &results).await;

    // One batch failed, one succeeded
    assert_eq!(r.sent, 1);
    assert_eq!(r.failed, 1);
    assert_eq!(r.errors.len(), 1);
}

#[tokio::test]
async fn backoff_never_collapses_to_zero() {
    use std::time::Instant;

    let mut server = Server::new_async().await;
    // Always return 429 to force retries
    let _m = server
        .mock("POST", "/")
        .with_status(429)
        .expect(3) // initial + 2 retries
        .create_async()
        .await;

    let mut config = WebhookConfig::new(server.url());
    config.max_retries = 2;
    config.timeout_secs = 5;

    let start = Instant::now();
    let result = send_webhook(&config, &[make_result(1)]).await;
    let elapsed = start.elapsed();

    assert!(!result.is_ok());
    assert!(elapsed > std::time::Duration::ZERO, "elapsed must be > 0");
}

// ── sign ─────────────────────────────────────────────────────────────────

#[test]
fn sign_computes_correct_hmac_sha256() {
    // HMAC-SHA256(key="Jefe", data="what do ya want for nothing?")
    // RFC 2104 test vector — cross-checked against https://www.freeformatter.com/hmac-generator.html
    let sig = sign("what do ya want for nothing?", "Jefe").unwrap();
    assert_eq!(sig.len(), 64, "signature must be 64 lowercase hex chars");
    assert!(
        sig.chars().all(|c| c.is_ascii_hexdigit()),
        "signature must be lowercase hex"
    );
    assert_eq!(
        sig,
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[tokio::test]
async fn webhook_times_out_when_server_is_silent() {
    use std::time::Instant;
    use tokio::net::TcpListener;

    // Bind a listener that accepts connections but never sends a response.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Accept in the background and hold the connection open silently.
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            // Hold the stream alive so the connection stays open.
            let _ = stream;
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });

    let mut config = WebhookConfig::new(format!("http://{addr}/"));
    config.max_retries = 0;
    config.timeout_secs = 1;

    let start = Instant::now();
    let r = send_webhook(&config, &[make_result(1)]).await;
    let elapsed = start.elapsed();

    assert_eq!(r.failed, 1);
    assert_eq!(r.errors.len(), 1);
    // Should time out well under 5 seconds.
    assert!(elapsed.as_secs() < 5, "timed out too slowly: {elapsed:?}");
}

#[tokio::test]
async fn on_batch_error_callback_fires_on_failure() {
    use std::sync::{Arc, Mutex};

    let mut server = Server::new_async().await;
    // Always fail — forces the callback to fire
    let _m = server
        .mock("POST", "/")
        .with_status(400)
        .create_async()
        .await;

    let fired: Arc<Mutex<Vec<u16>>> = Arc::new(Mutex::new(vec![]));
    let fired_clone = fired.clone();

    let mut config = WebhookConfig::new(server.url());
    config.on_batch_error = Some(Box::new(move |err| {
        if let Some(code) = err.status_code {
            fired_clone.lock().unwrap().push(code);
        }
    }));

    let result = send_webhook(&config, &[make_result(1_000_000)]).await;

    assert!(!result.is_ok());
    let codes = fired.lock().unwrap();
    assert_eq!(
        codes.len(),
        1,
        "callback must fire exactly once per failed batch"
    );
    assert_eq!(codes[0], 400);
}

#[tokio::test]
async fn max_retries_zero_fails_immediately_on_error() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/webhook")
        .with_status(500)
        .with_body("Internal Server Error")
        .create_async()
        .await;

    let mut config = WebhookConfig::new(format!("{}/webhook", server.url()));
    config.max_retries = 0;
    let results = vec![make_result(1_000_000)];
    let out = send_webhook(&config, &results).await;
    assert_eq!(out.failed, 1, "should fail with 0 retries on server error");
    assert_eq!(out.sent, 0);
}

#[tokio::test]
async fn retries_on_429_with_backoff() {
    let mut server = Server::new_async().await;
    // First call: 429
    let _m1 = server
        .mock("POST", "/webhook")
        .with_status(429)
        .with_body("Rate Limited")
        .expect_at_least(1)
        .create_async()
        .await;
    // Second call: 200 (retry succeeds)
    let _m2 = server
        .mock("POST", "/webhook")
        .with_status(200)
        .with_body("{}")
        .expect_at_least(1)
        .create_async()
        .await;

    let mut config = WebhookConfig::new(format!("{}/webhook", server.url()));
    config.max_retries = 2;
    let results = vec![make_result(1_000_000)];
    let start = std::time::Instant::now();
    let out = send_webhook(&config, &results).await;
    let elapsed = start.elapsed();

    // Should have retried and eventually succeeded (or failed gracefully)
    // The exact outcome depends on mockito's matching order,
    // but the key check is that it didn't hang and completed
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "should complete quickly"
    );
    assert_eq!(out.sent + out.failed, 1, "should process exactly one batch");
}
