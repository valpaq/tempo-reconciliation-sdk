use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::Args;
use serde_json::json;
use tempo_reconcile::{watch_tip20_transfers, WatchConfig};

/// Arguments for the `watch` subcommand.
#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Ethereum JSON-RPC endpoint URL.
    #[arg(long, env = "TEMPO_RPC_URL")]
    pub rpc: String,
    /// Chain ID (e.g. 42431 for Tempo Moderato testnet).
    #[arg(long, env = "TEMPO_CHAIN_ID")]
    pub chain_id: u32,
    /// TIP-20 token contract address (lowercase hex, "0x" prefixed).
    #[arg(long, env = "TEMPO_TOKEN")]
    pub token: String,
    /// Filter by sender address. Optional.
    #[arg(long)]
    pub from: Option<String>,
    /// Filter by recipient address. Optional.
    #[arg(long)]
    pub to: Option<String>,
    /// Starting block number. Defaults to the current chain tip.
    #[arg(long)]
    pub start_block: Option<u64>,
    /// Polling interval in milliseconds (minimum 100).
    #[arg(long, default_value_t = 1000)]
    pub poll_interval: u64,
    /// Also emit plain Transfer(from,to,amount) events that carry no memo.
    #[arg(long)]
    pub include_transfer_only: bool,
    /// Per-request RPC timeout in milliseconds (minimum 100). Default: 30000.
    #[arg(long, default_value_t = 30_000)]
    pub rpc_timeout: u64,
    /// Maximum block range per eth_getLogs call. Default: 2000.
    #[arg(long, default_value_t = 2000)]
    pub batch_size: u64,
    /// Output file path (appended to if it exists). Defaults to stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

fn is_valid_address(s: &str) -> bool {
    let hex = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => h,
        None => return false,
    };
    hex.len() == 40 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_args(args: &WatchArgs) -> Result<()> {
    let rpc_lower = args.rpc.to_ascii_lowercase();
    if !rpc_lower.starts_with("http://") && !rpc_lower.starts_with("https://") {
        anyhow::bail!(
            "--rpc must use http:// or https:// protocol, got: {}",
            args.rpc
        );
    }
    if args.chain_id == 0 {
        anyhow::bail!("--chain-id must be greater than 0");
    }
    if args.poll_interval < 100 {
        anyhow::bail!(
            "--poll-interval must be at least 100 ms, got {}",
            args.poll_interval
        );
    }
    if !is_valid_address(&args.token) {
        anyhow::bail!(
            "--token must be a 0x-prefixed 40-char hex address, got: {}",
            args.token
        );
    }
    if let Some(from) = &args.from {
        if !is_valid_address(from) {
            anyhow::bail!(
                "--from must be a 0x-prefixed 40-char hex address, got: {}",
                from
            );
        }
    }
    if let Some(to) = &args.to {
        if !is_valid_address(to) {
            anyhow::bail!(
                "--to must be a 0x-prefixed 40-char hex address, got: {}",
                to
            );
        }
    }
    if args.rpc_timeout < 100 {
        anyhow::bail!(
            "--rpc-timeout must be at least 100 ms, got {}",
            args.rpc_timeout
        );
    }
    if args.batch_size == 0 {
        anyhow::bail!("--batch-size must be at least 1");
    }
    Ok(())
}

/// Stream TIP-20 transfer events to stdout or a file until Ctrl+C.
///
/// Output is JSONL — one JSON object per line, in the same format accepted
/// by `run --events`, so the two commands compose naturally:
///
/// ```bash
/// tempo-reconcile watch --rpc $RPC --chain-id 42431 --token $TOKEN --out events.jsonl
/// tempo-reconcile run --events events.jsonl --expected invoices.csv
/// ```
pub async fn run_watch(args: &WatchArgs) -> Result<()> {
    validate_args(args)?;

    let config = WatchConfig {
        rpc_url: args.rpc.clone(),
        chain_id: args.chain_id,
        token: args.token.to_ascii_lowercase(),
        from: args.from.as_deref().map(|s| s.to_ascii_lowercase()),
        to: args.to.as_deref().map(|s| s.to_ascii_lowercase()),
        include_transfer_only: args.include_transfer_only,
        batch_size: args.batch_size,
        poll_interval_ms: args.poll_interval,
        dedup_ttl_secs: 60,
        dedup_max_size: 10_000,
        start_block: args.start_block,
        rpc_timeout_ms: args.rpc_timeout,
    };

    // Open writer: append to file or use stdout.
    let writer: Arc<Mutex<Box<dyn Write + Send>>> = match &args.out {
        Some(path) => {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("cannot open output file {}", path.display()))?;
            Arc::new(Mutex::new(Box::new(file)))
        }
        None => Arc::new(Mutex::new(Box::new(std::io::stdout()))),
    };

    let writer2 = Arc::clone(&writer);
    let count = Arc::new(AtomicU64::new(0));
    let count2 = Arc::clone(&count);

    let handle = watch_tip20_transfers(config, move |events| {
        for event in &events {
            // Serialize in EventRecord camelCase shape so output pipes into `run --events`.
            let line = json!({
                "chainId":    event.chain_id,
                "blockNumber": event.block_number,
                "txHash":     event.tx_hash,
                "logIndex":   event.log_index,
                "token":      event.token,
                "from":       event.from,
                "to":         event.to,
                "amount":     event.amount.to_string(),
                "memoRaw":    event.memo_raw,
                "timestamp":  event.timestamp,
            });
            let mut w = writer2.lock().unwrap_or_else(|e| e.into_inner());
            if let Err(e) = writeln!(w, "{line}") {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    break;
                }
                eprintln!("error: failed to write event: {e}");
                break;
            }
            count2.fetch_add(1, Ordering::Relaxed);
        }
    })
    .await
    .context("failed to connect to RPC")?;

    eprintln!("Watching for TIP-20 events. Press Ctrl+C to stop.");
    tokio::signal::ctrl_c().await?;

    let total = count.load(Ordering::Relaxed);
    handle.stop();
    eprintln!("Stopped. Captured {total} event(s).");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_args() -> WatchArgs {
        WatchArgs {
            rpc: "https://rpc.example.com".to_string(),
            chain_id: 42431,
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            from: None,
            to: None,
            start_block: None,
            poll_interval: 1000,
            include_transfer_only: false,
            rpc_timeout: 30_000,
            batch_size: 2000,
            out: None,
        }
    }

    #[test]
    fn valid_args_pass() {
        assert!(validate_args(&valid_args()).is_ok());
    }

    #[test]
    fn rejects_invalid_rpc_protocol() {
        let mut a = valid_args();
        a.rpc = "ftp://bad.com".to_string();
        assert!(validate_args(&a)
            .unwrap_err()
            .to_string()
            .contains("protocol"));
    }

    #[test]
    fn rejects_chain_id_zero() {
        let mut a = valid_args();
        a.chain_id = 0;
        assert!(validate_args(&a)
            .unwrap_err()
            .to_string()
            .contains("chain-id"));
    }

    #[test]
    fn rejects_poll_interval_below_100() {
        let mut a = valid_args();
        a.poll_interval = 50;
        assert!(validate_args(&a)
            .unwrap_err()
            .to_string()
            .contains("100 ms"));
    }

    #[test]
    fn rejects_invalid_token_address() {
        let mut a = valid_args();
        a.token = "not-an-address".to_string();
        assert!(validate_args(&a).unwrap_err().to_string().contains("token"));
    }

    #[test]
    fn rejects_invalid_to_address() {
        let mut a = valid_args();
        a.to = Some("0xshort".to_string());
        assert!(validate_args(&a).unwrap_err().to_string().contains("to"));
    }

    #[test]
    fn rejects_invalid_from_address() {
        let mut a = valid_args();
        a.from = Some("0xshort".to_string());
        assert!(validate_args(&a).unwrap_err().to_string().contains("from"));
    }

    #[test]
    fn accepts_valid_from_address() {
        let mut a = valid_args();
        a.from = Some("0xaaaa000000000000000000000000000000000001".to_string());
        assert!(validate_args(&a).is_ok());
    }

    #[test]
    fn rejects_ws_rpc_url() {
        let mut a = valid_args();
        a.rpc = "wss://rpc.example.com".to_string();
        assert!(validate_args(&a).is_err());
    }

    #[test]
    fn rejects_rpc_timeout_below_100() {
        let mut a = valid_args();
        a.rpc_timeout = 50;
        assert!(validate_args(&a)
            .unwrap_err()
            .to_string()
            .contains("rpc-timeout"));
    }

    #[test]
    fn accepts_rpc_timeout_100() {
        let mut a = valid_args();
        a.rpc_timeout = 100;
        assert!(validate_args(&a).is_ok());
    }

    #[test]
    fn batch_size_default_is_2000() {
        let a = valid_args();
        assert_eq!(a.batch_size, 2000);
    }

    #[test]
    fn batch_size_custom_value_accepted() {
        let mut a = valid_args();
        a.batch_size = 500;
        assert!(validate_args(&a).is_ok());
    }

    #[test]
    fn rejects_batch_size_zero() {
        let mut a = valid_args();
        a.batch_size = 0;
        assert!(validate_args(&a)
            .unwrap_err()
            .to_string()
            .contains("batch-size"));
    }
}
