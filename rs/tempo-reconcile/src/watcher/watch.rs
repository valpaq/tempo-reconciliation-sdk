use std::time::Duration;

use serde_json::json;
use tokio::sync::watch;
use tokio::time::sleep;

use super::decode::{decode_log, decode_transfer_log};
use super::dedup::DedupCache;
use super::rpc::RpcClient;
use crate::error::WatcherError;
use crate::types::PaymentEvent;

/// Configuration for [`get_tip20_transfer_history`] and [`watch_tip20_transfers`].
pub struct WatchConfig {
    /// Ethereum JSON-RPC endpoint URL.
    pub rpc_url: String,
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
    /// Maximum block range per `eth_getLogs` call. Default: 2000.
    pub batch_size: u64,
    /// Polling interval in milliseconds for [`watch_tip20_transfers`]. Default: 1000.
    pub poll_interval_ms: u64,
    /// Dedup cache TTL in seconds for [`watch_tip20_transfers`]. Default: 60.
    pub dedup_ttl_secs: u64,
    /// Maximum entries in the deduplication cache. Default: 10_000.
    pub dedup_max_size: usize,
    /// Starting block for [`watch_tip20_transfers`]. Defaults to the current chain tip.
    pub start_block: Option<u64>,
    /// Per-request timeout in milliseconds for RPC calls. Default: 30 000.
    pub rpc_timeout_ms: u64,
    /// Optional error callback invoked when RPC errors occur during polling.
    pub on_error: Option<Box<dyn Fn(WatcherError) + Send + Sync>>,
}

impl WatchConfig {
    pub fn new(rpc_url: impl Into<String>, chain_id: u32, token: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            chain_id,
            token: token.into(),
            from: None,
            to: None,
            include_transfer_only: false,
            batch_size: 2000,
            poll_interval_ms: 1000,
            dedup_ttl_secs: 60,
            dedup_max_size: 10_000,
            start_block: None,
            rpc_timeout_ms: 30_000,
            on_error: None,
        }
    }
}

/// Handle to a running [`watch_tip20_transfers`] task.
pub struct WatchHandle {
    stop: watch::Sender<bool>,
    handle: tokio::task::JoinHandle<()>,
}

impl WatchHandle {
    #[cfg_attr(not(feature = "watcher-ws"), allow(dead_code))]
    pub(super) fn new(stop: watch::Sender<bool>, handle: tokio::task::JoinHandle<()>) -> Self {
        Self { stop, handle }
    }

    /// Signal the watcher to stop at the next poll cycle.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }

    /// Wait for the watcher task to finish. Call [`stop`](Self::stop) first.
    pub async fn join(self) {
        let _ = self.handle.await;
    }

    /// Signal stop and wait for the task to finish.
    pub async fn stop_and_join(self) {
        self.stop();
        self.join().await;
    }
}

/// Fetch all `TransferWithMemo` events in the inclusive block range `[from_block, to_block]`.
///
/// Batches `eth_getLogs` calls by [`WatchConfig::batch_size`] blocks.
pub async fn get_tip20_transfer_history(
    config: &WatchConfig,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<PaymentEvent>, WatcherError> {
    let rpc = RpcClient::new(config.rpc_url.clone(), config.rpc_timeout_ms)?;
    let sig = event_sig();
    let batch = config.batch_size.max(1);
    let mut events = Vec::new();
    let mut start = from_block;

    while start <= to_block {
        let end = (start + batch - 1).min(to_block);
        let filter = build_filter(
            sig,
            &config.token,
            &config.from,
            &config.to,
            start,
            end,
            config.include_transfer_only,
        );
        let logs = rpc.get_logs(filter).await?;
        for log in &logs {
            if let Some(e) = decode_any_log(
                log,
                config.chain_id,
                &config.token,
                config.include_transfer_only,
            ) {
                events.push(e);
            }
        }
        start = end + 1;
    }

    Ok(events)
}

/// Start a polling watcher that calls `on_events` for each new batch of [`PaymentEvent`]s.
///
/// Returns a [`WatchHandle`]; call [`WatchHandle::stop`] to terminate the loop.
pub async fn watch_tip20_transfers<F>(
    config: WatchConfig,
    on_events: F,
) -> Result<WatchHandle, WatcherError>
where
    F: Fn(Vec<PaymentEvent>) + Send + Sync + 'static,
{
    let initial_tip = match config.start_block {
        Some(sb) => sb.saturating_sub(1),
        None => {
            RpcClient::new(config.rpc_url.clone(), config.rpc_timeout_ms)?
                .block_number()
                .await?
        }
    };
    let sig = event_sig();
    let (stop_tx, mut stop_rx) = watch::channel(false);

    let handle = tokio::spawn(async move {
        let rpc = match RpcClient::new(config.rpc_url.clone(), config.rpc_timeout_ms) {
            Ok(r) => r,
            Err(e) => {
                if let Some(ref cb) = config.on_error {
                    cb(WatcherError::Rpc(e.to_string()));
                }
                return;
            }
        };
        let mut tip = initial_tip;
        let mut cache = DedupCache::new(config.dedup_ttl_secs, config.dedup_max_size);

        loop {
            tokio::select! {
                _ = stop_rx.changed() => { break; }
                _ = sleep(Duration::from_millis(config.poll_interval_ms)) => {
                    let latest = match rpc.block_number().await {
                        Ok(b) => b,
                        Err(e) => {
                            if let Some(ref cb) = config.on_error { cb(e); }
                            continue;
                        }
                    };
                    if latest <= tip {
                        continue;
                    }
                    let from = tip + 1;
                    let to = latest.min(tip + config.batch_size.max(1));
                    let filter = build_filter(sig, &config.token, &config.from, &config.to, from, to, config.include_transfer_only);
                    match rpc.get_logs(filter).await {
                        Ok(logs) => {
                            let new_events: Vec<PaymentEvent> = logs
                                .iter()
                                .filter_map(|log| {
                                    decode_any_log(
                                        log,
                                        config.chain_id,
                                        &config.token,
                                        config.include_transfer_only,
                                    )
                                })
                                .filter(|e| {
                                    let key = format!("{}:{}", e.tx_hash, e.log_index);
                                    cache.check_and_insert(&key)
                                })
                                .collect();
                            if !new_events.is_empty() {
                                on_events(new_events);
                            }
                        }
                        Err(e) => {
                            if let Some(ref cb) = config.on_error { cb(e); }
                        }
                    }
                    tip = to;
                }
            }
        }
    });

    Ok(WatchHandle {
        stop: stop_tx,
        handle,
    })
}

/// keccak256("TransferWithMemo(address,address,uint256,bytes32)") as "0x" hex.
pub(super) fn event_sig() -> &'static str {
    use sha3::{Digest, Keccak256};
    use std::sync::OnceLock;
    static SIG: OnceLock<String> = OnceLock::new();
    SIG.get_or_init(|| {
        format!(
            "0x{}",
            hex::encode(Keccak256::digest(
                b"TransferWithMemo(address,address,uint256,bytes32)"
            ))
        )
    })
}

/// keccak256("Transfer(address,address,uint256)") as "0x" hex.
pub(super) fn transfer_event_sig() -> &'static str {
    use sha3::{Digest, Keccak256};
    use std::sync::OnceLock;
    static SIG: OnceLock<String> = OnceLock::new();
    SIG.get_or_init(|| {
        format!(
            "0x{}",
            hex::encode(Keccak256::digest(b"Transfer(address,address,uint256)"))
        )
    })
}

fn decode_any_log(
    log: &serde_json::Value,
    chain_id: u32,
    token: &str,
    include_transfer_only: bool,
) -> Option<PaymentEvent> {
    if include_transfer_only {
        let n = log["topics"].as_array().map(|a| a.len()).unwrap_or(0);
        if n >= 4 {
            decode_log(log, chain_id, token)
        } else {
            decode_transfer_log(log, chain_id, token)
        }
    } else {
        decode_log(log, chain_id, token)
    }
}

/// Build address filter topics (shared between HTTP and WS watchers).
pub(super) fn build_address_topics(
    from: &Option<String>,
    to: &Option<String>,
    topics: &mut Vec<serde_json::Value>,
) {
    if from.is_some() || to.is_some() {
        topics.push(match from {
            Some(addr) => serde_json::Value::String(format!(
                "0x{:0>64}",
                addr.strip_prefix("0x").unwrap_or(addr.as_str())
            )),
            None => serde_json::Value::Null,
        });
        if let Some(addr) = to {
            topics.push(serde_json::Value::String(format!(
                "0x{:0>64}",
                addr.strip_prefix("0x").unwrap_or(addr.as_str())
            )));
        }
    }
}

fn build_filter(
    sig: &str,
    token: &str,
    from: &Option<String>,
    to: &Option<String>,
    from_block: u64,
    to_block: u64,
    include_transfer_only: bool,
) -> serde_json::Value {
    let sig_value: serde_json::Value = if include_transfer_only {
        json!([sig, transfer_event_sig()])
    } else {
        json!(sig)
    };
    let mut topics: Vec<serde_json::Value> = vec![sig_value];
    build_address_topics(from, to, &mut topics);

    json!({
        "fromBlock": format!("0x{:x}", from_block),
        "toBlock":   format!("0x{:x}", to_block),
        "address":   token,
        "topics":    topics,
    })
}
