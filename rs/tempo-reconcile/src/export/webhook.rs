use std::time::Duration;

use rand::Rng;
use reqwest::Client;
use serde_json::{json, Value};

use crate::error::WebhookError;
use crate::types::{ExpectedPayment, MatchResult};

/// Callback type for batch delivery errors.
pub type BatchErrorCallback = Box<dyn Fn(&WebhookBatchError) + Send + Sync>;

/// Configuration for [`send_webhook`].
pub struct WebhookConfig {
    /// Webhook endpoint URL.
    pub url: String,
    /// HMAC-SHA256 signing secret. If `None`, the signature header is omitted.
    pub secret: Option<String>,
    /// Maximum results per HTTP POST. Default: 50.
    pub batch_size: usize,
    /// Maximum number of retries on transient failures. Default: 3.
    pub max_retries: u32,
    /// Per-request timeout in seconds. Default: 30.
    pub timeout_secs: u64,
    /// Optional callback invoked for every failed batch, after all retries are exhausted.
    pub on_batch_error: Option<BatchErrorCallback>,
}

impl WebhookConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            secret: None,
            batch_size: 50,
            max_retries: 3,
            timeout_secs: 30,
            on_batch_error: None,
        }
    }
}

/// A batch that failed to deliver.
#[derive(Debug)]
pub struct WebhookBatchError {
    /// The results that failed to deliver (enables caller retry).
    pub results: Vec<MatchResult>,
    /// HTTP status code, if an HTTP response was received.
    pub status_code: Option<u16>,
    /// Human-readable error description.
    pub error: String,
}

/// Aggregate result returned by [`send_webhook`].
#[derive(Debug)]
pub struct WebhookResult {
    /// Number of results successfully delivered.
    pub sent: usize,
    /// Number of results that failed to deliver.
    pub failed: usize,
    /// Per-batch errors for every failed batch.
    pub errors: Vec<WebhookBatchError>,
}

impl WebhookResult {
    /// Returns `true` if all batches were delivered without errors.
    pub fn is_ok(&self) -> bool {
        self.failed == 0
    }
}

/// POST `results` to a webhook URL in batches, signed with HMAC-SHA256.
///
/// Each batch is sent as `{ id, timestamp, events }`.
/// Headers:
/// - `X-Tempo-Reconcile-Signature` — HMAC-SHA256 of the body (hex), present when secret is set.
/// - `X-Tempo-Reconcile-Timestamp` — Unix seconds at batch creation time.
/// - `X-Tempo-Reconcile-Idempotency-Key` — stable across retries and process restarts for the same batch.
///
/// Retries on 429, 408, and 5xx with exponential backoff (cap: 30 s).
/// Non-retriable errors (4xx except 429/408) do not retry.
/// Returns a [`WebhookResult`] — all batches are attempted even if some fail.
pub async fn send_webhook(config: &WebhookConfig, results: &[MatchResult]) -> WebhookResult {
    if results.is_empty() {
        return WebhookResult {
            sent: 0,
            failed: 0,
            errors: vec![],
        };
    }

    let client = match Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return WebhookResult {
                sent: 0,
                failed: results.len(),
                errors: vec![WebhookBatchError {
                    results: results.to_vec(),
                    status_code: None,
                    error: e.to_string(),
                }],
            };
        }
    };

    let ts = current_ts();
    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();

    for chunk in results.chunks(config.batch_size.max(1)) {
        let fingerprint = batch_fingerprint(chunk);
        let body = build_body(chunk, ts, &fingerprint);
        match deliver(&client, config, &body, ts, &fingerprint).await {
            Ok(()) => sent += chunk.len(),
            Err((err, code)) => {
                failed += chunk.len();
                let e = WebhookBatchError {
                    results: chunk.to_vec(),
                    status_code: code,
                    error: err,
                };
                if let Some(cb) = &config.on_batch_error {
                    cb(&e);
                }
                errors.push(e);
            }
        }
    }

    WebhookResult {
        sent,
        failed,
        errors,
    }
}

fn current_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Stable fingerprint derived from batch content: keccak256("txHash:logIndex|...") as hex.
/// Identical across process restarts for the same set of events — used as idempotency key.
fn batch_fingerprint(results: &[MatchResult]) -> String {
    use sha3::{Digest, Keccak256};
    let fp: String = results
        .iter()
        .map(|r| format!("{}:{}", r.payment.tx_hash, r.payment.log_index))
        .collect::<Vec<_>>()
        .join("|");
    hex::encode(Keccak256::digest(fp.as_bytes()))
}

fn build_body(results: &[MatchResult], ts: u64, fingerprint: &str) -> String {
    let payload = json!({
        "id": format!("whevt_{}", fingerprint),
        "timestamp": ts,
        "events": results.iter().map(result_json).collect::<Vec<Value>>(),
    });
    // serde_json::to_string on a Value never returns an error
    serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
}

fn result_json(r: &MatchResult) -> Value {
    let p = &r.payment;
    json!({
        "status": r.status.as_str(),
        "payment": {
            "chainId":     p.chain_id,
            "blockNumber": p.block_number,
            "txHash":      p.tx_hash,
            "logIndex":    p.log_index,
            "token":       p.token,
            "from":        p.from,
            "to":          p.to,
            "amount":      p.amount.to_string(),
            "memoRaw":     p.memo_raw,
            "timestamp":   p.timestamp,
        },
        "expected": r.expected.as_ref().map(expected_json),
        "reason":          r.reason,
        "overpaidBy":      r.overpaid_by.map(|v| v.to_string()),
        "remainingAmount": r.remaining_amount.map(|v| v.to_string()),
        "isLate":          r.is_late,
    })
}

fn expected_json(e: &ExpectedPayment) -> Value {
    json!({
        "memoRaw": e.memo_raw,
        "token":   e.token,
        "to":      e.to,
        "amount":  e.amount.to_string(),
        "from":    e.from,
        "dueAt":   e.due_at,
        "meta":    e.meta,
    })
}

/// HMAC-SHA256(body, secret) as lowercase hex.
pub fn sign(body: &str, secret: &str) -> Result<String, WebhookError> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| WebhookError::Http(format!("HMAC init: {e}")))?;
    mac.update(body.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Deliver one batch with retries. Returns `Err((description, status_code))` on failure.
async fn deliver(
    client: &Client,
    config: &WebhookConfig,
    body: &str,
    ts: u64,
    fingerprint: &str,
) -> Result<(), (String, Option<u16>)> {
    // base_delay doubles each retry (1 → 2 → 4 → 8 → …, capped at 30s).
    // Jitter is applied to the sleep but does NOT affect the base progression.
    let mut base_delay = 1u64;
    let mut attempt = 0u32;

    loop {
        let is_last = attempt >= config.max_retries;

        let mut req = client
            .post(&config.url)
            .header("Content-Type", "application/json")
            .header("X-Tempo-Reconcile-Idempotency-Key", fingerprint)
            .header("X-Tempo-Reconcile-Timestamp", ts.to_string());

        if let Some(ref secret) = config.secret {
            match sign(body, secret) {
                Ok(sig) => {
                    req = req.header("X-Tempo-Reconcile-Signature", sig);
                }
                Err(e) => return Err((e.to_string(), None)),
            }
        }

        match req.body(body.to_string()).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(());
                }
                let code = status.as_u16();
                let retriable = matches!(code, 429 | 408) || status.is_server_error();
                if !retriable || is_last {
                    let msg = if retriable {
                        format!("HTTP {code} after {} retries", config.max_retries)
                    } else {
                        format!("HTTP {code}")
                    };
                    return Err((msg, Some(code)));
                }
            }
            Err(e) => {
                if is_last {
                    return Err((e.to_string(), None));
                }
            }
        }

        // Exponential backoff with jitter.
        let jittered = rand::thread_rng().gen_range(1..=base_delay.max(1));
        tokio::time::sleep(Duration::from_secs(jittered)).await;
        base_delay = (base_delay * 2).min(30);
        attempt += 1;
    }
}
