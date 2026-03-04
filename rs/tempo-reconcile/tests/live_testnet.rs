//! Live integration tests against Tempo Moderato testnet.
//!
//! These tests hit the real RPC at <https://rpc.moderato.tempo.xyz>
//! and verify that [`get_tip20_transfer_history`] works with actual chain data.
//!
//! Skipped in CI by default. Run with:
//! ```text
//! TEMPO_LIVE=1 cargo test --test live_testnet --features watcher -- --nocapture
//! ```

#![cfg(feature = "watcher")]

use tempo_reconcile::{get_tip20_transfer_history, WatchConfig};

const RPC_URL: &str = "https://rpc.moderato.tempo.xyz";
const CHAIN_ID: u32 = 42431;
const PATH_USD: &str = "0x20c0000000000000000000000000000000000000";

// Real TransferWithMemo event observed on Moderato testnet:
// block 6504870, tx 0xba01fd25..., logIndex 183
// from 0x51881fed... to 0x4489cdb6..., amount 50_000_000 (50 pathUSD)
const KNOWN_BLOCK: u64 = 6_504_870;
const KNOWN_TX: &str = "0xba01fd25c190087f10d6d6d921f2d4a3e0e7aafd21e92cbb7f56851060e3d3ba";
const KNOWN_FROM: &str = "0x51881fed631dae3f998dad2cf0c13e0a932cbb11";
const KNOWN_TO: &str = "0x4489cdb6f4574576058a579b86de27789c1cb8f3";
const KNOWN_AMOUNT: u128 = 50_000_000;

fn is_live() -> bool {
    std::env::var("TEMPO_LIVE").is_ok()
}

fn config() -> WatchConfig {
    WatchConfig::new(RPC_URL, CHAIN_ID, PATH_USD)
}

#[tokio::test]
async fn live_fetches_events_from_known_block() {
    if !is_live() {
        return;
    }
    let events = get_tip20_transfer_history(&config(), KNOWN_BLOCK, KNOWN_BLOCK)
        .await
        .expect("RPC call failed");

    assert!(
        !events.is_empty(),
        "block {KNOWN_BLOCK} should have at least one TransferWithMemo"
    );
    for ev in &events {
        assert_eq!(ev.chain_id, CHAIN_ID);
        assert_eq!(ev.token, PATH_USD);
        assert_eq!(ev.block_number, KNOWN_BLOCK);
    }
}

#[tokio::test]
async fn live_finds_known_transaction() {
    if !is_live() {
        return;
    }
    let events = get_tip20_transfer_history(&config(), KNOWN_BLOCK, KNOWN_BLOCK)
        .await
        .expect("RPC call failed");

    let found = events
        .iter()
        .find(|e| e.tx_hash.eq_ignore_ascii_case(KNOWN_TX));

    assert!(found.is_some(), "known tx {KNOWN_TX} not found in results");
    let ev = found.unwrap();
    assert_eq!(ev.from, KNOWN_FROM);
    assert_eq!(ev.to, KNOWN_TO);
    assert_eq!(ev.amount, KNOWN_AMOUNT);
    assert!(ev.memo_raw.is_some());
}

#[tokio::test]
async fn live_empty_result_for_unknown_recipient() {
    if !is_live() {
        return;
    }
    let mut cfg = config();
    cfg.to = Some("0x0000000000000000000000000000000000000001".to_string());
    let events = get_tip20_transfer_history(&cfg, KNOWN_BLOCK, KNOWN_BLOCK)
        .await
        .expect("RPC call failed");
    assert!(events.is_empty());
}

#[tokio::test]
async fn live_multi_block_range_with_batching() {
    if !is_live() {
        return;
    }
    let mut cfg = config();
    cfg.batch_size = 10; // force multiple eth_getLogs calls
    let events = get_tip20_transfer_history(&cfg, KNOWN_BLOCK, KNOWN_BLOCK + 30)
        .await
        .expect("RPC call failed");

    assert!(!events.is_empty());
    for ev in &events {
        assert!(ev.block_number >= KNOWN_BLOCK);
        assert!(ev.block_number <= KNOWN_BLOCK + 30);
    }
}
