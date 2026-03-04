#![cfg(feature = "explorer")]

use mockito::Server;
use serde_json::json;
use tempo_reconcile::explorer::{ExplorerClient, KnownEventPartValue};
use tempo_reconcile::ExplorerError;

#[test]
fn types_reexported_from_crate_root() {
    let _: Option<tempo_reconcile::AddressMetadata> = None;
    let _: Option<tempo_reconcile::TokenBalance> = None;
}

const ADDR: &str = "0x51881fed631dae3f998dad2cf0c13e0a932cbb11";

fn client(url: &str) -> ExplorerClient {
    ExplorerClient::new(url)
}

// ---------------------------------------------------------------------------
// get_metadata
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metadata_success() {
    let mut server = Server::new_async().await;
    let body = json!({
        "chainId": 42431,
        "accountType": "eoa",
        "txCount": 42,
        "lastActivityTimestamp": 1_700_000_000_u64,
        "createdTimestamp": 1_600_000_000_u64,
        "createdTxHash": "0xabc",
        "createdBy": "0xdef"
    })
    .to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let meta = client(&server.url()).get_metadata(ADDR).await.unwrap();

    assert_eq!(meta.address, ADDR);
    assert_eq!(meta.chain_id, 42431);
    assert_eq!(meta.account_type, "eoa");
    assert_eq!(meta.tx_count, 42);
    assert_eq!(meta.last_activity_timestamp, 1_700_000_000);
    assert_eq!(meta.created_timestamp, 1_600_000_000);
    assert_eq!(meta.created_tx_hash.as_deref(), Some("0xabc"));
    assert_eq!(meta.created_by.as_deref(), Some("0xdef"));
}

#[tokio::test]
async fn metadata_missing_optional_fields_defaults() {
    let mut server = Server::new_async().await;
    let body = json!({"accountType": "contract", "txCount": 0}).to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let meta = client(&server.url()).get_metadata(ADDR).await.unwrap();
    assert_eq!(meta.account_type, "contract");
    assert_eq!(meta.tx_count, 0);
    assert!(meta.created_tx_hash.is_none());
    assert!(meta.created_by.is_none());
}

#[tokio::test]
async fn metadata_not_found_returns_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(404)
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();

    assert!(matches!(err, ExplorerError::NotFound(_)));
}

#[tokio::test]
async fn metadata_server_error_returns_http_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(500)
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();

    assert!(matches!(err, ExplorerError::Http(_)));
}

#[tokio::test]
async fn metadata_unauthorized_returns_http_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(401)
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();
    assert!(matches!(err, ExplorerError::Http(_)));
}

#[tokio::test]
async fn metadata_forbidden_returns_http_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(403)
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();
    assert!(matches!(err, ExplorerError::Http(_)));
}

#[tokio::test]
async fn metadata_malformed_json_returns_parse_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("not valid json {{{")
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();
    assert!(matches!(err, ExplorerError::Parse(_)));
}

// ---------------------------------------------------------------------------
// get_balances
// ---------------------------------------------------------------------------

#[tokio::test]
async fn balances_parses_response() {
    let mut server = Server::new_async().await;
    let body = json!({
        "balances": [
            {
                "token": "0x20c0000000000000000000000000000000000000",
                "name": "pathUSD",
                "symbol": "pathUSD",
                "currency": "USD",
                "decimals": 6,
                "balance": "1000000"
            },
            {
                "token": "0xabc0000000000000000000000000000000000000",
                "name": "",
                "symbol": "",
                "currency": "",
                "decimals": 6,
                "balance": "500000"
            }
        ]
    })
    .to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/balances/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let resp = client(&server.url()).get_balances(ADDR).await.unwrap();

    assert_eq!(resp.balances.len(), 2);
    assert_eq!(
        resp.balances[0].token,
        "0x20c0000000000000000000000000000000000000"
    );
    assert_eq!(resp.balances[0].symbol, "pathUSD");
    assert_eq!(resp.balances[0].currency, "USD");
    assert_eq!(resp.balances[0].decimals, 6);
    assert_eq!(resp.balances[0].balance, "1000000");
    assert_eq!(resp.balances[1].balance, "500000");
}

#[tokio::test]
async fn balances_empty_on_404() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/balances/".to_string()),
        )
        .with_status(404)
        .create_async()
        .await;

    let resp = client(&server.url()).get_balances(ADDR).await.unwrap();
    assert!(resp.balances.is_empty());
}

// ---------------------------------------------------------------------------
// get_history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_parses_response() {
    let mut server = Server::new_async().await;
    let body = json!({
        "transactions": [{
            "hash": "0xdeadbeef",
            "blockNumber": "6504870",
            "timestamp": 1_700_000_000_u64,
            "from": "0xaaaa000000000000000000000000000000000001",
            "to":   "0xbbbb000000000000000000000000000000000002",
            "value": "0",
            "status": "success",
            "gasUsed": "21000",
            "effectiveGasPrice": "1000000000",
            "knownEvents": [{
                "type": "TransferWithMemo",
                "note": "transfer",
                "parts": [
                    {"type": "action", "value": "sent"},
                    {"type": "amount", "value": {
                        "token": "0x20c0",
                        "value": "50.000000",
                        "decimals": 6,
                        "symbol": "pathUSD"
                    }}
                ],
                "meta": {"memo": "0x01abc"}
            }]
        }],
        "total": 42,
        "offset": 0,
        "limit": 20,
        "hasMore": true,
        "countCapped": false,
        "error": null
    })
    .to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/history/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let resp = client(&server.url())
        .get_history(ADDR, None, None)
        .await
        .unwrap();

    assert_eq!(resp.total, 42);
    assert_eq!(resp.offset, 0);
    assert_eq!(resp.limit, 20);
    assert!(resp.has_more);
    assert!(!resp.count_capped);
    assert!(resp.error.is_none());

    assert_eq!(resp.transactions.len(), 1);
    let tx = &resp.transactions[0];
    assert_eq!(tx.hash, "0xdeadbeef");
    assert_eq!(tx.block_number, "6504870");
    assert_eq!(tx.timestamp, 1_700_000_000);
    assert_eq!(tx.status, "success");
    assert_eq!(tx.gas_used, "21000");
    assert_eq!(tx.effective_gas_price, "1000000000");

    assert_eq!(tx.known_events.len(), 1);
    let ev = &tx.known_events[0];
    assert_eq!(ev.event_type, "TransferWithMemo");
    assert_eq!(ev.note.as_deref(), Some("transfer"));
    assert_eq!(ev.parts.len(), 2);
    assert_eq!(ev.parts[0].part_type, "action");
    assert!(matches!(&ev.parts[0].value, KnownEventPartValue::Text(s) if s == "sent"));
    assert!(
        matches!(&ev.parts[1].value, KnownEventPartValue::Amount { symbol, .. } if symbol == "pathUSD")
    );
    assert_eq!(
        ev.meta.as_ref().unwrap().get("memo").map(String::as_str),
        Some("0x01abc")
    );
}

#[tokio::test]
async fn history_empty_on_404() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/history/".to_string()),
        )
        .with_status(404)
        .create_async()
        .await;

    let resp = client(&server.url())
        .get_history(ADDR, None, None)
        .await
        .unwrap();
    assert!(resp.transactions.is_empty());
    assert_eq!(resp.total, 0);
    assert!(!resp.has_more);
}

#[tokio::test]
async fn history_server_error_returns_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/history/".to_string()),
        )
        .with_status(500)
        .create_async()
        .await;

    let err = client(&server.url())
        .get_history(ADDR, None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, ExplorerError::Http(_)));
}

#[tokio::test]
async fn history_limit_and_offset_appended_to_url() {
    let mut server = Server::new_async().await;
    let empty_page = json!({
        "transactions": [],
        "total": 0,
        "offset": 20,
        "limit": 10,
        "hasMore": false,
        "countCapped": false,
        "error": null
    })
    .to_string();

    // Match URLs containing both limit=10 and offset=20 in any order.
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"(limit=10.*offset=20|offset=20.*limit=10)".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&empty_page)
        .create_async()
        .await;

    let resp = client(&server.url())
        .get_history(ADDR, Some(10), Some(20))
        .await
        .unwrap();
    assert!(resp.transactions.is_empty());
    _m.assert_async().await;
}

#[tokio::test]
async fn addresses_normalized_to_lowercase() {
    let mut server = Server::new_async().await;
    let body = json!({
        "transactions": [{
            "hash": "0x1",
            "blockNumber": "1",
            "timestamp": 0,
            "from": "0xAAAA000000000000000000000000000000000001",
            "to":   "0xBBBB000000000000000000000000000000000002",
            "value": "0",
            "status": "success",
            "gasUsed": "21000",
            "effectiveGasPrice": "1",
            "knownEvents": []
        }],
        "total": 1,
        "offset": 0,
        "limit": 20,
        "hasMore": false,
        "countCapped": false,
        "error": null
    })
    .to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/history/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let resp = client(&server.url())
        .get_history(ADDR, None, None)
        .await
        .unwrap();
    assert_eq!(
        resp.transactions[0].from,
        "0xaaaa000000000000000000000000000000000001"
    );
    assert_eq!(
        resp.transactions[0].to,
        "0xbbbb000000000000000000000000000000000002"
    );
}

#[tokio::test]
async fn history_second_page_returns_correct_offset() {
    let mut server = Server::new_async().await;

    let page1 = json!({
        "transactions": [{"hash":"0x1","blockNumber":"1","timestamp":1000,"from":"0xf","to":"0xa","value":"0","status":"success","gasUsed":"21000","effectiveGasPrice":"1","knownEvents":[]}],
        "total": 2, "offset": 0, "limit": 1,
        "hasMore": true, "countCapped": false, "error": null
    })
    .to_string();
    let page2 = json!({
        "transactions": [{"hash":"0x2","blockNumber":"2","timestamp":2000,"from":"0xf","to":"0xa","value":"0","status":"success","gasUsed":"21000","effectiveGasPrice":"1","knownEvents":[]}],
        "total": 2, "offset": 1, "limit": 1,
        "hasMore": false, "countCapped": false, "error": null
    })
    .to_string();

    let _m1 = server
        .mock("GET", mockito::Matcher::Regex(r"offset=0".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&page1)
        .create_async()
        .await;
    let _m2 = server
        .mock("GET", mockito::Matcher::Regex(r"offset=1".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&page2)
        .create_async()
        .await;

    let r1 = client(&server.url())
        .get_history(ADDR, Some(1), Some(0))
        .await
        .unwrap();
    let r2 = client(&server.url())
        .get_history(ADDR, Some(1), Some(1))
        .await
        .unwrap();

    assert_eq!(r1.transactions[0].hash, "0x1");
    assert!(r1.has_more);
    assert_eq!(r2.transactions[0].hash, "0x2");
    assert!(!r2.has_more);
    assert_eq!(r1.total, r2.total);
}

#[tokio::test]
async fn history_part_with_null_value_decoded_as_empty_text() {
    let mut server = Server::new_async().await;
    let body = json!({
        "transactions": [{
            "hash": "0x1", "blockNumber": "1", "timestamp": 0,
            "from": "0xf", "to": "0xa", "value": "0",
            "status": "success", "gasUsed": "0", "effectiveGasPrice": "0",
            "knownEvents": [{
                "type": "unknown", "note": null,
                "parts": [
                    {"type": "action", "value": null},
                    {"type": "count",  "value": 42}
                ],
                "meta": null
            }]
        }],
        "total": 1, "offset": 0, "limit": 20,
        "hasMore": false, "countCapped": false, "error": null
    })
    .to_string();

    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/history/".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(&body)
        .create_async()
        .await;

    let resp = client(&server.url())
        .get_history(ADDR, None, None)
        .await
        .unwrap();

    let ev = &resp.transactions[0].known_events[0];
    assert_eq!(ev.parts.len(), 2);
    // null → Text("")
    assert!(matches!(&ev.parts[0].value, KnownEventPartValue::Text(s) if s.is_empty()));
    // integer (not object, not string) → Text("")
    assert!(matches!(&ev.parts[1].value, KnownEventPartValue::Text(s) if s.is_empty()));
    assert!(ev.meta.is_none());
    assert!(ev.note.is_none());
}

#[tokio::test]
async fn metadata_rate_limited_returns_http_err() {
    let mut server = Server::new_async().await;
    let _m = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/address/metadata/".to_string()),
        )
        .with_status(429)
        .create_async()
        .await;

    let err = client(&server.url()).get_metadata(ADDR).await.unwrap_err();
    assert!(matches!(err, ExplorerError::Http(_)));
}
