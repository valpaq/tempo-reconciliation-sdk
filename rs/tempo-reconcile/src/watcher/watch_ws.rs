use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::watch;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::decode::{decode_log, decode_transfer_log};
use super::dedup::DedupCache;
use crate::error::WatcherError;
use crate::types::PaymentEvent;

/// Configuration for [`watch_tip20_transfers_ws`].
pub struct WatchWsConfig {
    /// WebSocket RPC endpoint URL (`wss://` or `ws://`).
    pub ws_url: String,
    /// Chain ID (e.g. `42431` for Tempo Moderato testnet).
    pub chain_id: u32,
    /// TIP-20 token contract address (lowercase hex, "0x" prefixed).
    pub token: String,
    /// If set, only include transfers from this sender address.
    pub from: Option<String>,
    /// If set, only include transfers to this recipient address.
    pub to: Option<String>,
    /// If true, also emit plain `Transfer(from, to, amount)` events (no memo). Default: false.
    pub include_transfer_only: bool,
    /// Maximum reconnection attempts. Default: 5 (`0` = unlimited).
    pub max_reconnects: u32,
    /// Base reconnect delay in milliseconds. Doubles per attempt, capped at 30 s. Default: 1000.
    pub reconnect_delay_ms: u64,
    /// Dedup cache TTL in seconds. Default: 60.
    pub dedup_ttl_secs: u64,
    /// Maximum entries in the deduplication cache. Default: 10_000.
    pub dedup_max_size: usize,
    /// Timeout in milliseconds for receiving any WS message (ping, data, close).
    /// If no message arrives within this window the session errors and reconnects.
    /// Default: 30 000.
    pub read_timeout_ms: u64,
}

impl WatchWsConfig {
    pub fn new(ws_url: impl Into<String>, chain_id: u32, token: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
            chain_id,
            token: token.into(),
            from: None,
            to: None,
            include_transfer_only: false,
            max_reconnects: 5,
            reconnect_delay_ms: 1_000,
            dedup_ttl_secs: 60,
            dedup_max_size: 10_000,
            read_timeout_ms: 30_000,
        }
    }
}

/// Start a WebSocket push watcher that calls `on_events` for each new batch of events.
///
/// Subscribes to `eth_subscribe` logs for `TransferWithMemo` events and automatically
/// reconnects with exponential backoff (base `reconnect_delay_ms`, capped at 30 s).
///
/// Returns a [`super::watch::WatchHandle`] to stop the watcher.
pub async fn watch_tip20_transfers_ws<F>(
    config: WatchWsConfig,
    on_events: F,
) -> Result<super::watch::WatchHandle, WatcherError>
where
    F: Fn(Vec<PaymentEvent>) + Send + Sync + 'static,
{
    let (stop_tx, mut stop_rx) = watch::channel(false);

    let handle = tokio::spawn(async move {
        let mut attempts = 0u32;
        let mut cache = DedupCache::new(config.dedup_ttl_secs, config.dedup_max_size);

        loop {
            if *stop_rx.borrow() {
                break;
            }

            match run_session(&config, &on_events, &mut cache, &mut stop_rx).await {
                Ok(()) => break, // clean stop
                Err(_) => {
                    attempts += 1;
                    if config.max_reconnects > 0 && attempts > config.max_reconnects {
                        break;
                    }
                    let delay = (config.reconnect_delay_ms * (1u64 << attempts.min(5))).min(30_000);
                    tokio::select! {
                        _ = stop_rx.changed() => break,
                        _ = sleep(Duration::from_millis(delay)) => {}
                    }
                }
            }
        }
    });

    Ok(super::watch::WatchHandle::new(stop_tx, handle))
}

type WsSink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;
type WsStream = futures_util::stream::SplitStream<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
>;

/// Connect, send eth_subscribe, and return (write, read, subscription_id).
async fn subscribe_handshake(
    config: &WatchWsConfig,
) -> Result<(WsSink, WsStream, String), WatcherError> {
    use super::watch::{build_address_topics, event_sig, transfer_event_sig};

    let (ws_stream, _) = connect_async(&config.ws_url)
        .await
        .map_err(|e| WatcherError::Ws(e.to_string()))?;

    let (mut write, mut read) = ws_stream.split();

    let sig = event_sig();
    let topic0: Value = if config.include_transfer_only {
        json!([sig, transfer_event_sig()])
    } else {
        Value::String(sig.to_string())
    };
    let mut topics: Vec<Value> = vec![topic0];
    build_address_topics(&config.from, &config.to, &mut topics);

    let sub_req = json!({
        "jsonrpc": "2.0",
        "method": "eth_subscribe",
        "params": ["logs", {
            "address": config.token,
            "topics": topics,
        }],
        "id": 1
    });

    write
        .send(Message::Text(sub_req.to_string()))
        .await
        .map_err(|e| WatcherError::Ws(e.to_string()))?;

    let sub_id: String = timeout(Duration::from_millis(config.read_timeout_ms), async {
        loop {
            match read.next().await {
                Some(Ok(Message::Text(text))) => {
                    let v: Value =
                        serde_json::from_str(&text).map_err(|e| WatcherError::Ws(e.to_string()))?;
                    if v["id"] == 1 {
                        let id = v["result"]
                            .as_str()
                            .ok_or_else(|| WatcherError::Ws("no subscription id".into()))?
                            .to_string();
                        break Ok::<String, WatcherError>(id);
                    }
                }
                Some(Ok(Message::Ping(data))) => {
                    let _ = write.send(Message::Pong(data)).await;
                }
                Some(Err(e)) => break Err(WatcherError::Ws(e.to_string())),
                None => {
                    break Err(WatcherError::Ws(
                        "connection closed before subscribe".into(),
                    ))
                }
                _ => {}
            }
        }
    })
    .await
    .map_err(|_| WatcherError::Ws("subscribe timeout".into()))??;

    Ok((write, read, sub_id))
}

async fn run_session<F>(
    config: &WatchWsConfig,
    on_events: &F,
    cache: &mut DedupCache,
    stop_rx: &mut watch::Receiver<bool>,
) -> Result<(), WatcherError>
where
    F: Fn(Vec<PaymentEvent>) + Send + Sync + 'static,
{
    let (mut write, mut read, sub_id) = subscribe_handshake(config).await?;

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    return Ok(());
                }
            }
            result = timeout(Duration::from_millis(config.read_timeout_ms), read.next()) => {
                let msg = match result {
                    Err(_) => return Err(WatcherError::Ws("idle timeout".into())),
                    Ok(m) => m,
                };
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let v: Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };

                        // eth_subscription push: {"method":"eth_subscription","params":{"subscription":"0x...","result":{...}}}
                        if v["method"].as_str() != Some("eth_subscription") {
                            continue;
                        }
                        if v["params"]["subscription"].as_str() != Some(&sub_id) {
                            continue;
                        }
                        let log = &v["params"]["result"];
                        let n = log["topics"].as_array().map(|a| a.len()).unwrap_or(0);
                        let parsed = if config.include_transfer_only && n < 4 {
                            decode_transfer_log(log, config.chain_id, &config.token)
                        } else {
                            decode_log(log, config.chain_id, &config.token)
                        };
                        if let Some(event) = parsed {
                            let key = format!("{}:{}", event.tx_hash, event.log_index);
                            if cache.check_and_insert(&key) {
                                on_events(vec![event]);
                            }
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return Err(WatcherError::Ws("connection closed".into()));
                    }
                    Some(Err(e)) => return Err(WatcherError::Ws(e.to_string())),
                    _ => {}
                }
            }
        }
    }
}
