use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};

use crate::error::WatcherError;

pub(super) struct RpcClient {
    client: Client,
    url: String,
}

impl RpcClient {
    pub(super) fn new(url: String, timeout_ms: u64) -> Result<Self, WatcherError> {
        let client = Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| WatcherError::Http(format!("failed to build client: {e}")))?;
        Ok(Self { client, url })
    }

    pub(super) async fn block_number(&self) -> Result<u64, WatcherError> {
        let resp = self
            .call(json!({
                "jsonrpc": "2.0",
                "method": "eth_blockNumber",
                "params": [],
                "id": 1
            }))
            .await?;
        let hex = resp["result"]
            .as_str()
            .ok_or_else(|| WatcherError::Rpc("eth_blockNumber: missing result".into()))?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| WatcherError::Rpc(e.to_string()))
    }

    pub(super) async fn get_logs(&self, filter: Value) -> Result<Vec<Value>, WatcherError> {
        let resp = self
            .call(json!({
                "jsonrpc": "2.0",
                "method": "eth_getLogs",
                "params": [filter],
                "id": 1
            }))
            .await?;
        resp["result"]
            .as_array()
            .cloned()
            .ok_or_else(|| WatcherError::Rpc("eth_getLogs: missing result".into()))
    }

    async fn call(&self, body: Value) -> Result<Value, WatcherError> {
        let max_retries = 3u32;
        let mut attempt = 0u32;

        loop {
            let is_last = attempt >= max_retries;

            let response = self
                .client
                .post(&self.url)
                .json(&body)
                .send()
                .await
                .map_err(|e| WatcherError::Http(e.to_string()))?;

            let status = response.status();
            if status.as_u16() == 429 || status.is_server_error() {
                if is_last {
                    return Err(WatcherError::Http(format!("HTTP {status} after retries")));
                }
                let retry_secs = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1)
                    .min(60);
                tokio::time::sleep(Duration::from_secs(retry_secs)).await;
                attempt += 1;
                continue;
            }
            if !status.is_success() {
                return Err(WatcherError::Http(format!("HTTP {status}")));
            }

            let resp = response
                .json::<Value>()
                .await
                .map_err(|e| WatcherError::Http(e.to_string()))?;

            if let Some(err) = resp.get("error") {
                return Err(WatcherError::Rpc(err.to_string()));
            }
            return Ok(resp);
        }
    }
}
