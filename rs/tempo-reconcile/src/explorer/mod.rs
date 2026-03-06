//! Tempo Explorer REST client.
//!
//! Provides metadata, token balances, and full transaction history from the Tempo Explorer API.
//! Enabled with `feature = "explorer"`.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use tempo_reconcile::explorer::ExplorerClient;
//!
//! let client = ExplorerClient::new("https://explore.tempo.xyz/api");
//! let meta = client.get_metadata("0x4489cdb6f4574576058a579b86de27789c1cb8f3").await?;
//! println!("{:?}", meta);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::time::Duration;

use reqwest::Client;

use crate::error::ExplorerError;

/// Default Tempo Explorer API base URL.
pub const DEFAULT_BASE_URL: &str = "https://explore.tempo.xyz/api";

/// Metadata for an on-chain address.
#[derive(Debug, Clone)]
pub struct AddressMetadata {
    /// The queried address (lowercase, `0x`-prefixed).
    pub address: String,
    /// Chain ID.
    pub chain_id: u32,
    /// Account type returned by the Explorer (e.g. `"eoa"`, `"contract"`).
    pub account_type: String,
    /// Total number of transactions sent from this address.
    pub tx_count: u64,
    /// Unix timestamp (seconds) of last on-chain activity.
    pub last_activity_timestamp: u64,
    /// Unix timestamp (seconds) of account creation / first activity.
    pub created_timestamp: u64,
    /// Transaction hash of the first inbound transfer, if available.
    pub created_tx_hash: Option<String>,
    /// Address that originated the first inbound transfer, if available.
    pub created_by: Option<String>,
}

/// TIP-20 token balance for an address.
#[derive(Debug, Clone)]
pub struct TokenBalance {
    /// Token contract address (lowercase, `0x`-prefixed).
    pub token: String,
    /// Token name (e.g. `"pathUSD"`).
    pub name: String,
    /// Token symbol (e.g. `"pathUSD"`).
    pub symbol: String,
    /// Currency code (e.g. `"USD"`).
    pub currency: String,
    /// Decimals — always 6 for TIP-20 tokens.
    pub decimals: u8,
    /// Raw balance as string (bigint-safe, matches Explorer API format).
    pub balance: String,
}

/// Balances response wrapper (mirrors the Explorer API `{ "balances": [...] }` shape).
#[derive(Debug, Clone)]
pub struct BalancesResponse {
    /// Token balances held by the queried address.
    pub balances: Vec<TokenBalance>,
}

/// Value of one part within a [`KnownEvent`].
///
/// Either a plain string (for `"action"`, `"text"`, `"account"` parts) or a structured
/// token-amount value (for `"amount"` parts).
#[derive(Debug, Clone)]
pub enum KnownEventPartValue {
    /// Plain text value.
    Text(String),
    /// Structured token amount.
    Amount {
        /// Token contract address.
        token: String,
        /// Human-readable value string.
        value: String,
        /// Token decimals.
        decimals: u8,
        /// Token symbol.
        symbol: String,
    },
}

/// One display part of a [`KnownEvent`].
#[derive(Debug, Clone)]
pub struct KnownEventPart {
    /// Part type: `"action"`, `"amount"`, `"text"`, or `"account"`.
    pub part_type: String,
    /// Part value — plain string or structured token amount.
    pub value: KnownEventPartValue,
}

/// A decoded on-chain event attached to a transaction.
#[derive(Debug, Clone)]
pub struct KnownEvent {
    /// Event type string (e.g. `"TransferWithMemo"`).
    pub event_type: String,
    /// Optional human-readable note.
    pub note: Option<String>,
    /// Display parts for UI rendering.
    pub parts: Vec<KnownEventPart>,
    /// Arbitrary key-value metadata attached to the event.
    pub meta: Option<HashMap<String, String>>,
}

/// A full transaction from the Explorer history endpoint.
#[derive(Debug, Clone)]
pub struct ExplorerTransaction {
    /// Transaction hash (`0x`-prefixed hex).
    pub hash: String,
    /// Block number as string (bigint-safe).
    pub block_number: String,
    /// Unix timestamp in seconds.
    pub timestamp: u64,
    /// Sender address (lowercase).
    pub from: String,
    /// Recipient address (lowercase).
    pub to: String,
    /// Transaction value as string.
    pub value: String,
    /// Transaction status (e.g. `"success"`, `"failed"`).
    pub status: String,
    /// Gas used as string.
    pub gas_used: String,
    /// Effective gas price as string.
    pub effective_gas_price: String,
    /// Decoded events attached to this transaction.
    pub known_events: Vec<KnownEvent>,
}

/// Paginated response from [`ExplorerClient::get_history`].
#[derive(Debug, Clone)]
pub struct HistoryResponse {
    /// Transactions in this page.
    pub transactions: Vec<ExplorerTransaction>,
    /// Total number of matching transactions (may be capped).
    pub total: u64,
    /// Pagination offset used for this response.
    pub offset: u64,
    /// Page size used for this response.
    pub limit: u64,
    /// Whether more results exist beyond this page.
    pub has_more: bool,
    /// Whether the total count is capped by the API.
    pub count_capped: bool,
    /// API-level error string, if any.
    pub error: Option<String>,
}

/// REST client for the Tempo Explorer API.
pub struct ExplorerClient {
    client: Client,
    base_url: String,
    timeout_ms: u64,
}

impl ExplorerClient {
    /// Create a client pointing at `base_url` (e.g. `"https://explore.tempo.xyz/api"`).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            timeout_ms: 30_000,
        }
    }

    /// Create a client using the default Tempo Explorer API URL.
    pub fn default_mainnet() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }

    /// Fetch metadata for `address`.
    pub async fn get_metadata(&self, address: &str) -> Result<AddressMetadata, ExplorerError> {
        let url = format!(
            "{}/address/metadata/{}",
            self.base_url,
            address.to_lowercase()
        );
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_millis(self.timeout_ms))
            .send()
            .await
            .map_err(|e| ExplorerError::Http(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ExplorerError::NotFound(address.to_string()));
        }
        if !resp.status().is_success() {
            return Err(ExplorerError::Http(format!(
                "status {}",
                resp.status().as_u16()
            )));
        }

        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExplorerError::Parse(e.to_string()))?;

        let chain_id = v["chainId"].as_u64().unwrap_or(0) as u32;
        let account_type = v["accountType"]
            .as_str()
            .ok_or_else(|| ExplorerError::Parse("missing or invalid field: accountType".into()))?
            .to_string();
        let tx_count = v["txCount"]
            .as_u64()
            .ok_or_else(|| ExplorerError::Parse("missing or invalid field: txCount".into()))?;

        Ok(AddressMetadata {
            address: address.to_lowercase(),
            chain_id,
            account_type,
            tx_count,
            last_activity_timestamp: v["lastActivityTimestamp"].as_u64().unwrap_or(0),
            created_timestamp: v["createdTimestamp"].as_u64().unwrap_or(0),
            created_tx_hash: v["createdTxHash"].as_str().map(str::to_string),
            created_by: v["createdBy"].as_str().map(str::to_string),
        })
    }

    /// Fetch all TIP-20 token balances held by `address`.
    pub async fn get_balances(&self, address: &str) -> Result<BalancesResponse, ExplorerError> {
        let url = format!(
            "{}/address/balances/{}",
            self.base_url,
            address.to_lowercase()
        );
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_millis(self.timeout_ms))
            .send()
            .await
            .map_err(|e| ExplorerError::Http(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(BalancesResponse { balances: vec![] });
        }
        if !resp.status().is_success() {
            return Err(ExplorerError::Http(format!(
                "status {}",
                resp.status().as_u16()
            )));
        }

        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExplorerError::Parse(e.to_string()))?;

        let items = v["balances"].as_array().cloned().unwrap_or_default();
        let mut balances = Vec::with_capacity(items.len());
        for item in &items {
            balances.push(TokenBalance {
                token: item["token"].as_str().unwrap_or_default().to_lowercase(),
                name: item["name"].as_str().unwrap_or_default().to_string(),
                symbol: item["symbol"].as_str().unwrap_or_default().to_string(),
                currency: item["currency"].as_str().unwrap_or_default().to_string(),
                decimals: item["decimals"].as_u64().unwrap_or(6) as u8,
                balance: item["balance"].as_str().unwrap_or("0").to_string(),
            });
        }
        Ok(BalancesResponse { balances })
    }

    /// Fetch paginated transaction history for `address`.
    ///
    /// Use `limit` and `offset` for pagination. Both default to the API's own defaults when
    /// `None` (typically `limit=20`, `offset=0`).
    pub async fn get_history(
        &self,
        address: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<HistoryResponse, ExplorerError> {
        let mut params: Vec<String> = Vec::new();
        if let Some(l) = limit {
            params.push(format!("limit={l}"));
        }
        if let Some(o) = offset {
            params.push(format!("offset={o}"));
        }
        let query = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };

        let url = format!(
            "{}/address/history/{}{}",
            self.base_url,
            address.to_lowercase(),
            query
        );

        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_millis(self.timeout_ms))
            .send()
            .await
            .map_err(|e| ExplorerError::Http(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(HistoryResponse {
                transactions: vec![],
                total: 0,
                offset: 0,
                limit: 0,
                has_more: false,
                count_capped: false,
                error: None,
            });
        }
        if !resp.status().is_success() {
            return Err(ExplorerError::Http(format!(
                "status {}",
                resp.status().as_u16()
            )));
        }

        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ExplorerError::Parse(e.to_string()))?;

        let txs_raw = v["transactions"].as_array().cloned().unwrap_or_default();
        let transactions = txs_raw.iter().map(parse_transaction).collect();

        Ok(HistoryResponse {
            transactions,
            total: v["total"].as_u64().unwrap_or(0),
            offset: v["offset"].as_u64().unwrap_or(0),
            limit: v["limit"].as_u64().unwrap_or(0),
            has_more: v["hasMore"].as_bool().unwrap_or(false),
            count_capped: v["countCapped"].as_bool().unwrap_or(false),
            error: v["error"].as_str().map(str::to_string),
        })
    }
}

fn parse_transaction(tx: &serde_json::Value) -> ExplorerTransaction {
    let known_events = tx["knownEvents"]
        .as_array()
        .map(|arr| arr.iter().map(parse_known_event).collect())
        .unwrap_or_default();

    ExplorerTransaction {
        hash: tx["hash"].as_str().unwrap_or_default().to_string(),
        block_number: tx["blockNumber"].as_str().unwrap_or_default().to_string(),
        timestamp: tx["timestamp"].as_u64().unwrap_or(0),
        from: tx["from"].as_str().unwrap_or_default().to_lowercase(),
        to: tx["to"].as_str().unwrap_or_default().to_lowercase(),
        value: tx["value"].as_str().unwrap_or("0").to_string(),
        status: tx["status"].as_str().unwrap_or_default().to_string(),
        gas_used: tx["gasUsed"].as_str().unwrap_or("0").to_string(),
        effective_gas_price: tx["effectiveGasPrice"].as_str().unwrap_or("0").to_string(),
        known_events,
    }
}

fn parse_known_event(ev: &serde_json::Value) -> KnownEvent {
    let parts = ev["parts"]
        .as_array()
        .map(|arr| arr.iter().map(parse_known_event_part).collect())
        .unwrap_or_default();

    let meta = ev["meta"].as_object().map(|obj| {
        obj.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect::<HashMap<String, String>>()
    });

    KnownEvent {
        event_type: ev["type"].as_str().unwrap_or_default().to_string(),
        note: ev["note"].as_str().map(str::to_string),
        parts,
        meta,
    }
}

fn parse_known_event_part(part: &serde_json::Value) -> KnownEventPart {
    let part_type = part["type"].as_str().unwrap_or_default().to_string();
    let value = if part["value"].is_object() {
        KnownEventPartValue::Amount {
            token: part["value"]["token"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            value: part["value"]["value"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            decimals: part["value"]["decimals"].as_u64().unwrap_or(6) as u8,
            symbol: part["value"]["symbol"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        }
    } else {
        KnownEventPartValue::Text(part["value"].as_str().unwrap_or_default().to_string())
    };
    KnownEventPart { part_type, value }
}

/// Convenience factory — equivalent to [`ExplorerClient::default_mainnet`].
///
/// Matches the TypeScript SDK's `createExplorerClient()` factory function.
pub fn create_explorer_client() -> ExplorerClient {
    ExplorerClient::default_mainnet()
}
