#![cfg(feature = "watcher")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mockito::Server;
use serde_json::json;
use sha3::{Digest, Keccak256};
use tempo_reconcile::{get_tip20_transfer_history, watch_tip20_transfers, WatchConfig};

fn event_sig() -> String {
    format!(
        "0x{}",
        hex::encode(Keccak256::digest(
            b"TransferWithMemo(address,address,uint256,bytes32)"
        ))
    )
}

fn make_log(from: &str, to: &str, amount_hex: &str, memo: &str, block: &str) -> serde_json::Value {
    let from_padded = format!("0x{:0>64}", from.strip_prefix("0x").unwrap_or(from));
    let to_padded = format!("0x{:0>64}", to.strip_prefix("0x").unwrap_or(to));
    json!({
        "topics": [
            event_sig(),
            from_padded,
            to_padded,
            memo,
        ],
        "data": amount_hex,
        "blockNumber": block,
        "transactionHash": "0xdeadbeef000000000000000000000000000000000000000000000000000000001",
        "logIndex": "0x0",
    })
}

#[tokio::test]
async fn get_history_returns_empty_when_no_logs() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":[]}"#)
        .create_async()
        .await;

    let config = WatchConfig::new(
        server.url(),
        42431,
        "0x20c0000000000000000000000000000000000000",
    );
    let events = get_tip20_transfer_history(&config, 100, 200).await.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn get_history_decodes_single_log() {
    let mut server = Server::new_async().await;

    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        "0xaaaa000000000000000000000000000000000001",
        "0xbbbb000000000000000000000000000000000002",
        &format!("0x{:0>64x}", 10_000_000u128),
        memo,
        "0x64",
    );
    let body = json!({"jsonrpc":"2.0","id":1,"result":[log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let config = WatchConfig::new(
        server.url(),
        42431,
        "0x20c0000000000000000000000000000000000000",
    );
    let events = get_tip20_transfer_history(&config, 100, 100).await.unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].amount, 10_000_000);
    assert_eq!(events[0].block_number, 100);
    assert_eq!(events[0].from, "0xaaaa000000000000000000000000000000000001");
    assert_eq!(events[0].to, "0xbbbb000000000000000000000000000000000002");
    assert_eq!(events[0].memo_raw.as_deref(), Some(memo));
}

#[tokio::test]
async fn get_history_batches_by_batch_size() {
    let mut server = Server::new_async().await;

    // batch_size = 50 over range 0-99 → 2 calls
    let empty = r#"{"jsonrpc":"2.0","id":1,"result":[]}"#;
    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(empty)
        .expect(2)
        .create_async()
        .await;

    let mut config = WatchConfig::new(server.url(), 42431, "0xtoken");
    config.batch_size = 50;
    let events = get_tip20_transfer_history(&config, 0, 99).await.unwrap();
    assert!(events.is_empty());
    _m.assert_async().await;
}

#[tokio::test]
async fn get_history_rpc_error_propagates() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"server error"}}"#)
        .create_async()
        .await;

    let config = WatchConfig::new(server.url(), 42431, "0xtoken");
    assert!(get_tip20_transfer_history(&config, 0, 10).await.is_err());
}

#[tokio::test]
async fn get_history_skips_malformed_logs() {
    let mut server = Server::new_async().await;

    // log with only 2 topics (invalid)
    let bad_log = json!({
        "topics": ["0xsig", "0xfrom"],
        "data": "0x0",
        "blockNumber": "0x1",
        "transactionHash": "0xabcd",
        "logIndex": "0x0",
    });
    let body = json!({"jsonrpc":"2.0","id":1,"result":[bad_log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let config = WatchConfig::new(server.url(), 42431, "0xtoken");
    let events = get_tip20_transfer_history(&config, 0, 10).await.unwrap();
    assert!(events.is_empty());
}

// ---------------------------------------------------------------------------
// watch_tip20_transfers (polling loop) tests
// ---------------------------------------------------------------------------

/// Minimal HTTP/1.1 server that serves each response once in sequence,
/// then repeats the last response for all subsequent requests.
/// Uses `Connection: close` so reqwest opens a new TCP connection per call.
async fn sequential_server(responses: Vec<String>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let responses = Arc::new(responses);
    let idx = Arc::new(AtomicUsize::new(0));

    tokio::spawn(async move {
        loop {
            let Ok((mut conn, _)) = listener.accept().await else {
                break;
            };
            let responses = responses.clone();
            let idx = idx.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let _ = conn.read(&mut buf).await;

                let i = idx.fetch_add(1, Ordering::SeqCst);
                let body = responses
                    .get(i)
                    .or_else(|| responses.last())
                    .cloned()
                    .unwrap_or_default();

                let http = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = conn.write_all(http.as_bytes()).await;
            });
        }
    });

    format!("http://127.0.0.1:{}", port)
}

fn block_number_resp(hex_block: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":1,"result":"{}"}}"#, hex_block)
}

fn get_logs_resp(logs: serde_json::Value) -> String {
    json!({"jsonrpc":"2.0","id":1,"result":logs}).to_string()
}

#[tokio::test]
async fn watch_starts_and_stops_cleanly() {
    let url = sequential_server(vec![
        block_number_resp("0x64"), // initial block = 100
        block_number_resp("0x64"), // poll: no advance
    ])
    .await;

    let mut config = WatchConfig::new(url, 42431, "0xtoken");
    config.poll_interval_ms = 5;

    let handle = watch_tip20_transfers(config, |_| {}).await.unwrap();
    // Small delay to let the poll loop run at least once
    tokio::time::sleep(Duration::from_millis(20)).await;
    handle.stop();
    // If we get here without hanging, test passes
}

#[tokio::test]
async fn watch_returns_error_when_initial_block_fails() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/")
        .with_status(500)
        .create_async()
        .await;

    let config = WatchConfig::new(server.url(), 42431, "0xtoken");
    let result = watch_tip20_transfers(config, |_| {}).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn watch_fires_callback_when_new_block_available() {
    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        "0xaaaa000000000000000000000000000000000001",
        "0xbbbb000000000000000000000000000000000002",
        &format!("0x{:0>64x}", 5_000_000u128),
        memo,
        "0x65",
    );

    let url = sequential_server(vec![
        block_number_resp("0x64"), // initial = 100
        block_number_resp("0x65"), // poll: latest = 101 > 100 → fetch logs
        get_logs_resp(json!([log])),
        block_number_resp("0x65"), // subsequent polls: no advance
    ])
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut config = WatchConfig::new(url, 42431, "0x20c0000000000000000000000000000000000000");
    config.poll_interval_ms = 5;

    let handle = watch_tip20_transfers(config, move |events| {
        for e in events {
            let _ = tx.try_send(e);
        }
    })
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout waiting for callback")
        .expect("channel closed");

    handle.stop();
    assert_eq!(event.amount, 5_000_000);
    assert_eq!(event.block_number, 101);
    assert_eq!(event.from, "0xaaaa000000000000000000000000000000000001");
}

#[tokio::test]
async fn watch_deduplicates_repeated_events() {
    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        "0xaaaa000000000000000000000000000000000001",
        "0xbbbb000000000000000000000000000000000002",
        &format!("0x{:0>64x}", 1_000_000u128),
        memo,
        "0x66",
    );
    let logs_body = get_logs_resp(json!([log]));

    // Two consecutive polls both return the same log (simulates RPC returning same block range twice)
    let url = sequential_server(vec![
        block_number_resp("0x65"), // initial = 101
        block_number_resp("0x66"), // poll 1: latest = 102 → fetch logs
        logs_body.clone(),
        block_number_resp("0x67"), // poll 2: latest = 103 → fetch logs (same log again)
        logs_body,
        block_number_resp("0x67"), // no more advance
    ])
    .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    let tx2 = tx.clone();

    let mut config = WatchConfig::new(url, 42431, "0x20c0000000000000000000000000000000000000");
    config.poll_interval_ms = 5;

    let handle = watch_tip20_transfers(config, move |events| {
        counter_clone.fetch_add(events.len(), Ordering::SeqCst);
        for e in &events {
            let _ = tx2.try_send(e.tx_hash.clone());
        }
    })
    .await
    .unwrap();

    // Wait for first event
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    // Give the second poll time to run
    tokio::time::sleep(Duration::from_millis(30)).await;
    handle.stop();

    // Same tx_hash:log_index should fire the callback only once
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "event should be delivered exactly once"
    );
}

#[tokio::test]
async fn get_history_with_from_filter_decodes_matching_event() {
    let mut server = Server::new_async().await;

    let from_addr = "0xaaaa000000000000000000000000000000000001";
    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        from_addr,
        "0xbbbb000000000000000000000000000000000002",
        &format!("0x{:0>64x}", 5_000_000u128),
        memo,
        "0x64",
    );
    let body = json!({"jsonrpc":"2.0","id":1,"result":[log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let mut config = WatchConfig::new(
        server.url(),
        42431,
        "0x20c0000000000000000000000000000000000000",
    );
    config.from = Some(from_addr.to_string());

    let events = get_tip20_transfer_history(&config, 100, 100).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, from_addr);
    assert_eq!(events[0].amount, 5_000_000);
}

#[tokio::test]
async fn include_transfer_only_also_returns_plain_transfer_events() {
    let mut server = Server::new_async().await;

    let transfer_sig = format!(
        "0x{}",
        hex::encode(Keccak256::digest(b"Transfer(address,address,uint256)"))
    );
    let from_padded = "0x000000000000000000000000aaaa000000000000000000000000000000000001";
    let to_padded = "0x000000000000000000000000bbbb000000000000000000000000000000000002";
    let amount_hex = format!("0x{:0>64x}", 7_000_000u128);

    let plain_transfer_log = json!({
        "topics": [transfer_sig, from_padded, to_padded],
        "data": amount_hex,
        "blockNumber": "0x64",
        "transactionHash": "0xcafebabe0000000000000000000000000000000000000000000000000000001",
        "logIndex": "0x0",
    });
    let body = json!({"jsonrpc":"2.0","id":1,"result":[plain_transfer_log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let mut config = WatchConfig::new(
        server.url(),
        42431,
        "0x20c0000000000000000000000000000000000000",
    );
    config.include_transfer_only = true;

    let events = get_tip20_transfer_history(&config, 100, 100).await.unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, "0xaaaa000000000000000000000000000000000001");
    assert_eq!(events[0].to, "0xbbbb000000000000000000000000000000000002");
    assert_eq!(events[0].amount, 7_000_000);
    assert!(events[0].memo_raw.is_none());
    assert!(events[0].memo.is_none());
}

#[tokio::test]
async fn get_history_errors_on_rpc_timeout() {
    use tokio::net::TcpListener;
    // Accept TCP connection but never respond — simulates a hung node.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (_conn, _) = listener.accept().await.unwrap();
        // Hold connection open so reqwest doesn't see EOF.
        tokio::time::sleep(Duration::from_secs(60)).await;
    });
    let url = format!("http://127.0.0.1:{}", port);
    let mut config = WatchConfig::new(url, 42431, "0xtoken");
    config.rpc_timeout_ms = 200;
    let result = get_tip20_transfer_history(&config, 0, 10).await;
    assert!(result.is_err());
}

const TOKEN: &str = "0x20c0000000000000000000000000000000000000";

#[tokio::test]
async fn watcher_waits_on_429_retry_after() {
    let mut server = Server::new_async().await;

    // eth_getLogs call → 429 with Retry-After: 1
    let _m1 = server
        .mock("POST", "/")
        .with_status(429)
        .with_header("Retry-After", "1")
        .create_async()
        .await;

    let start = std::time::Instant::now();
    let config = WatchConfig::new(server.url(), 42431, TOKEN);
    let result = get_tip20_transfer_history(&config, 1, 1).await;
    let elapsed = start.elapsed();

    assert!(result.is_err(), "429 should return Err");
    assert!(
        elapsed >= Duration::from_secs(1),
        "should have waited Retry-After seconds, elapsed={elapsed:?}"
    );
}

#[tokio::test]
async fn combined_from_and_to_filters_returns_matching_event() {
    // Build a log matching both from=SENDER_A and to=RECIP_B.
    // The server returns the single matching log; we assert 1 event decoded.
    let mut server = Server::new_async().await;

    let sender = "0xaaaa000000000000000000000000000000000001";
    let recip = "0xbbbb000000000000000000000000000000000002";
    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        sender,
        recip,
        &format!("0x{:0>64x}", 3_000_000u128),
        memo,
        "0x64",
    );
    let body = json!({"jsonrpc":"2.0","id":1,"result":[log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let mut config = WatchConfig::new(server.url(), 42431, TOKEN);
    config.from = Some(sender.to_string());
    config.to = Some(recip.to_string());

    let events = get_tip20_transfer_history(&config, 100, 100).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].from, sender);
    assert_eq!(events[0].to, recip);
    assert_eq!(events[0].amount, 3_000_000);
}

#[tokio::test]
async fn get_history_with_to_filter_only_returns_matching_event() {
    // Verify that setting only config.to (without config.from) still works:
    // build_filter should emit null for topic[1] (from) and the padded recipient for topic[2].
    let mut server = Server::new_async().await;

    let recip = "0xbbbb000000000000000000000000000000000002";
    let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let log = make_log(
        "0xaaaa000000000000000000000000000000000001",
        recip,
        &format!("0x{:0>64x}", 2_000_000u128),
        memo,
        "0x64",
    );
    let body = json!({"jsonrpc":"2.0","id":1,"result":[log]}).to_string();

    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let mut config = WatchConfig::new(server.url(), 42431, TOKEN);
    config.to = Some(recip.to_string());
    // config.from is intentionally left as None

    let events = get_tip20_transfer_history(&config, 100, 100).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].to, recip);
    assert_eq!(events[0].amount, 2_000_000);
}

#[tokio::test]
async fn get_history_rpc_returns_malformed_json_propagates_error() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("this is not valid json {{{")
        .create_async()
        .await;

    let config = WatchConfig::new(server.url(), 42431, "0xtoken");
    let result = get_tip20_transfer_history(&config, 0, 10).await;
    assert!(result.is_err(), "malformed JSON must propagate as Err");
}
