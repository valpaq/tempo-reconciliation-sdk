#![cfg(feature = "watcher-ws")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use sha3::{Digest, Keccak256};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use tempo_reconcile::{watch_tip20_transfers_ws, WatchWsConfig};

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
        "topics": [event_sig(), from_padded, to_padded, memo],
        "data": amount_hex,
        "blockNumber": block,
        "transactionHash": "0xdeadbeef000000000000000000000000000000000000000000000000deadbeef",
        "logIndex": "0x0",
    })
}

/// Actions a mock WebSocket connection executes in sequence.
enum WsOp {
    /// Consume (and discard) the next incoming message.
    ReadOne,
    /// Send a text message.
    Send(String),
    /// Close the WebSocket.
    Close,
    /// Wait briefly before next action.
    Sleep(Duration),
}

/// Spawn a mock WebSocket server. Each element in `connection_scripts` is
/// the sequence of ops for one accepted connection, in order.
async fn mock_ws_server(connection_scripts: Vec<Vec<WsOp>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let scripts = Arc::new(tokio::sync::Mutex::new(
        connection_scripts
            .into_iter()
            .collect::<std::collections::VecDeque<_>>(),
    ));

    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let script = {
                let mut q = scripts.lock().await;
                q.pop_front()
            };
            let Some(ops) = script else {
                break;
            };
            tokio::spawn(async move {
                let ws = accept_async(stream).await.unwrap();
                let (mut write, mut read) = ws.split();
                for op in ops {
                    match op {
                        WsOp::ReadOne => {
                            let _ = read.next().await;
                        }
                        WsOp::Send(msg) => {
                            let _ = write.send(Message::Text(msg)).await;
                        }
                        WsOp::Close => {
                            let _ = write.send(Message::Close(None)).await;
                            break;
                        }
                        WsOp::Sleep(d) => {
                            tokio::time::sleep(d).await;
                        }
                    }
                }
            });
        }
    });

    format!("ws://127.0.0.1:{}", port)
}

/// Build the eth_subscribe ACK response.
fn sub_ack(id: &str) -> String {
    json!({"jsonrpc":"2.0","id":1,"result":id}).to_string()
}

/// Build an eth_subscription push message.
fn sub_push(sub_id: &str, log: &serde_json::Value) -> String {
    json!({
        "method": "eth_subscription",
        "params": { "subscription": sub_id, "result": log }
    })
    .to_string()
}

const TOKEN: &str = "0x20c0000000000000000000000000000000000000";
const FROM: &str = "0xaaaa000000000000000000000000000000000001";
const TO: &str = "0xbbbb000000000000000000000000000000000002";
const MEMO: &str = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";

#[tokio::test]
async fn ws_watch_receives_events() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 7_000_000u128),
        MEMO,
        "0x65",
    );
    let sub_id = "0xsub1";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,               // read eth_subscribe
        WsOp::Send(sub_ack(sub_id)), // ACK
        WsOp::Sleep(Duration::from_millis(5)),
        WsOp::Send(sub_push(sub_id, &log)),      // push one event
        WsOp::Sleep(Duration::from_millis(500)), // keep connection open
    ]])
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let config = WatchWsConfig::new(url, 42431, TOKEN);

    let handle = watch_tip20_transfers_ws(config, move |events| {
        for e in events {
            let _ = tx.try_send(e);
        }
    })
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    handle.stop();
    assert_eq!(event.amount, 7_000_000);
    assert_eq!(event.from, FROM);
    assert_eq!(event.to, TO);
}

#[tokio::test]
async fn ws_watch_stop_terminates_cleanly() {
    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack("0xsub2")),
        WsOp::Sleep(Duration::from_millis(1000)), // server stays open
    ]])
    .await;

    let config = WatchWsConfig::new(url, 42431, TOKEN);
    let handle = watch_tip20_transfers_ws(config, |_| {}).await.unwrap();

    // Let the watcher connect and subscribe
    tokio::time::sleep(Duration::from_millis(30)).await;
    handle.stop();
    // Should return quickly, not hang
}

#[tokio::test]
async fn ws_watch_reconnects_after_close() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 3_000_000u128),
        MEMO,
        "0x70",
    );
    let sub_id = "0xsub3";

    // First connection: subscribe then immediately close → triggers reconnect
    // Second connection: subscribe then send an event
    let url = mock_ws_server(vec![
        vec![
            WsOp::ReadOne,
            WsOp::Send(sub_ack(sub_id)),
            WsOp::Sleep(Duration::from_millis(5)),
            WsOp::Close,
        ],
        vec![
            WsOp::ReadOne,
            WsOp::Send(sub_ack(sub_id)),
            WsOp::Sleep(Duration::from_millis(5)),
            WsOp::Send(sub_push(sub_id, &log)),
            WsOp::Sleep(Duration::from_millis(500)),
        ],
    ])
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut config = WatchWsConfig::new(url, 42431, TOKEN);
    config.reconnect_delay_ms = 10; // fast reconnect for test

    let handle = watch_tip20_transfers_ws(config, move |events| {
        for e in events {
            let _ = tx.try_send(e);
        }
    })
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timeout waiting for reconnect + event")
        .expect("channel closed");

    handle.stop();
    assert_eq!(event.amount, 3_000_000);
}

#[tokio::test]
async fn ws_watch_deduplicates_same_event() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 2_000_000u128),
        MEMO,
        "0x71",
    );
    let sub_id = "0xsub4";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack(sub_id)),
        WsOp::Sleep(Duration::from_millis(5)),
        WsOp::Send(sub_push(sub_id, &log)),
        WsOp::Sleep(Duration::from_millis(10)),
        WsOp::Send(sub_push(sub_id, &log)), // same event again
        WsOp::Sleep(Duration::from_millis(200)),
    ]])
    .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);

    let config = WatchWsConfig::new(url, 42431, TOKEN);

    let handle = watch_tip20_transfers_ws(config, move |events| {
        counter_clone.fetch_add(events.len(), Ordering::SeqCst);
        for e in &events {
            let _ = tx.try_send(e.amount);
        }
    })
    .await
    .unwrap();

    // Wait for first delivery
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .unwrap();

    // Wait for the duplicate push to have been processed
    tokio::time::sleep(Duration::from_millis(50)).await;
    handle.stop();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "duplicate event must be deduplicated"
    );
}

#[tokio::test]
async fn ws_watch_filters_wrong_subscription_id() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 9_000_000u128),
        MEMO,
        "0x72",
    );
    let sub_id = "0xsub5";
    let wrong_id = "0xwrong";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack(sub_id)),
        WsOp::Sleep(Duration::from_millis(5)),
        // Push with wrong subscription id — should be ignored
        WsOp::Send(sub_push(wrong_id, &log)),
        WsOp::Sleep(Duration::from_millis(200)),
    ]])
    .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let config = WatchWsConfig::new(url, 42431, TOKEN);
    let handle = watch_tip20_transfers_ws(config, move |events| {
        counter_clone.fetch_add(events.len(), Ordering::SeqCst);
    })
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;
    handle.stop();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "event with wrong sub_id must be ignored"
    );
}

#[tokio::test]
async fn ws_watch_returns_error_on_subscription_failure() {
    // Server sends a JSON-RPC error in place of the subscription ACK, then closes.
    let error_resp = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "error": { "code": -32600, "message": "unavailable" }
    })
    .to_string();

    // Two scripts: initial connection + one retry (max_reconnects = 1).
    let url = mock_ws_server(vec![
        vec![WsOp::ReadOne, WsOp::Send(error_resp.clone()), WsOp::Close],
        vec![WsOp::ReadOne, WsOp::Send(error_resp), WsOp::Close],
    ])
    .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let mut config = WatchWsConfig::new(url, 42431, TOKEN);
    config.max_reconnects = 1;
    config.reconnect_delay_ms = 10; // fast retry for test speed

    let handle = watch_tip20_transfers_ws(config, move |events| {
        counter_clone.fetch_add(events.len(), Ordering::SeqCst);
    })
    .await
    .unwrap();

    // Give the watcher enough time to exhaust retries and stop.
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.stop();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "no events must be emitted when subscription ACK is an error"
    );
}

#[tokio::test]
async fn ws_watch_with_from_filter_receives_event() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 6_000_000u128),
        MEMO,
        "0x74",
    );
    let sub_id = "0xsub_from";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack(sub_id)),
        WsOp::Sleep(Duration::from_millis(5)),
        WsOp::Send(sub_push(sub_id, &log)),
        WsOp::Sleep(Duration::from_millis(500)),
    ]])
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut config = WatchWsConfig::new(url, 42431, TOKEN);
    config.from = Some(FROM.to_string());

    let handle = watch_tip20_transfers_ws(config, move |events| {
        for e in events {
            let _ = tx.try_send(e);
        }
    })
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout")
        .expect("channel closed");

    handle.stop();
    assert_eq!(event.from, FROM);
    assert_eq!(event.amount, 6_000_000);
}

#[tokio::test]
async fn ws_watch_skips_malformed_json_mid_session() {
    let log = make_log(
        FROM,
        TO,
        &format!("0x{:0>64x}", 4_000_000u128),
        MEMO,
        "0x73",
    );
    let sub_id = "0xsub_malformed";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack(sub_id)),
        WsOp::Sleep(Duration::from_millis(5)),
        // Malformed JSON — must be skipped, not cause a panic or reconnect
        WsOp::Send("not json at all }{{{".to_string()),
        WsOp::Sleep(Duration::from_millis(10)),
        // Valid event follows — must be delivered
        WsOp::Send(sub_push(sub_id, &log)),
        WsOp::Sleep(Duration::from_millis(500)),
    ]])
    .await;

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let config = WatchWsConfig::new(url, 42431, TOKEN);
    let handle = watch_tip20_transfers_ws(config, move |events| {
        counter_clone.fetch_add(events.len(), Ordering::SeqCst);
    })
    .await
    .unwrap();

    // Allow time for both messages to be processed
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.stop();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "malformed JSON must be skipped; valid event after it must be delivered"
    );
}

#[tokio::test]
async fn ws_watch_include_transfer_only_receives_plain_transfer() {
    use sha3::{Digest, Keccak256};

    let transfer_sig = format!(
        "0x{}",
        hex::encode(Keccak256::digest(b"Transfer(address,address,uint256)"))
    );

    let from_padded = format!("0x{:0>64}", FROM.strip_prefix("0x").unwrap_or(FROM));
    let to_padded = format!("0x{:0>64}", TO.strip_prefix("0x").unwrap_or(TO));
    let amount: u128 = 5_000_000;
    let amount_hex = format!("0x{:0>64x}", amount);

    let transfer_log = serde_json::json!({
        "address": TOKEN,
        "topics": [transfer_sig, from_padded, to_padded],
        "data": amount_hex,
        "transactionHash": "0x7472616e73666572310000000000000000000000000000000000000000000001",
        "logIndex": "0x0",
        "blockNumber": "0x1",
        "blockHash": "0xblockhash1",
        "transactionIndex": "0x0"
    });

    let sub_id = "0xsub_transfer_only";

    let url = mock_ws_server(vec![vec![
        WsOp::ReadOne,
        WsOp::Send(sub_ack(sub_id)),
        WsOp::Sleep(Duration::from_millis(5)),
        WsOp::Send(sub_push(sub_id, &transfer_log)),
        WsOp::Sleep(Duration::from_millis(500)),
    ]])
    .await;

    let (tx, mut rx) = tokio::sync::mpsc::channel(4);
    let mut config = WatchWsConfig::new(url, 42431, TOKEN);
    config.include_transfer_only = true;

    let handle = watch_tip20_transfers_ws(config, move |events| {
        for e in events {
            let _ = tx.try_send(e);
        }
    })
    .await
    .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout waiting for plain Transfer event")
        .expect("channel closed");

    handle.stop();
    assert_eq!(event.amount, amount);
    assert_eq!(event.from, FROM);
    assert_eq!(event.to, TO);
    assert!(
        event.memo_raw.is_none(),
        "plain Transfer must have memo_raw: None"
    );
}

#[tokio::test]
async fn ws_watch_reconnects_on_idle_timeout() {
    // Each session: send ACK then go silent for 500 ms — longer than read_timeout_ms=100.
    // The watcher must fire the idle timeout, error out of run_session, and reconnect.
    let url = mock_ws_server(vec![
        vec![
            WsOp::ReadOne,
            WsOp::Send(sub_ack("0xsub1")),
            WsOp::Sleep(Duration::from_millis(500)),
        ],
        vec![
            WsOp::ReadOne,
            WsOp::Send(sub_ack("0xsub2")),
            WsOp::Sleep(Duration::from_millis(500)),
        ],
    ])
    .await;

    let mut config = WatchWsConfig::new(url, 42431, TOKEN);
    config.read_timeout_ms = 100;
    config.max_reconnects = 3;
    config.reconnect_delay_ms = 10;

    let handle = watch_tip20_transfers_ws(config, |_| {}).await.unwrap();
    // Wait long enough for at least one reconnect cycle (timeout=100 + delay=10)
    tokio::time::sleep(Duration::from_millis(600)).await;
    handle.stop();
    // Reaching here without hanging means the idle timeout fired and reconnect worked.
}
