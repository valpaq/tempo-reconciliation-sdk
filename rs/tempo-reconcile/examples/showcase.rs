//! End-to-end reconciliation showcase — runs offline, no RPC required.
//!
//! Usage: `cargo run --example showcase`

use tempo_reconcile::{
    decode_memo, encode_memo_v1, export_csv, export_json, is_memo_v1, issuer_tag_from_namespace,
    random_salt, EncodeMemoV1Params, ExpectedPayment, MatchStatus, MemoType, PaymentEvent,
    Reconciler, ReconcilerOptions,
};

fn main() {
    // 1. Derive a deterministic issuer tag for your namespace.
    let issuer_tag = issuer_tag_from_namespace("my-company");
    println!("issuer tag: {} (0x{:016x})", issuer_tag, issuer_tag);

    // 2. Encode a structured memo (bytes32).
    let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag,
        ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
        salt: Some(random_salt()),
    })
    .expect("encode");

    println!("memo:       {}", memo_raw);
    println!("is v1:      {}", is_memo_v1(&memo_raw));

    if let Some(m) = decode_memo(&memo_raw) {
        println!("decoded:    {:?}", m);
    }

    // 3. Register the expected payment.
    let mut reconciler = Reconciler::new(ReconcilerOptions {
        allow_partial: true,
        amount_tolerance_bps: 50, // 0.5% tolerance
        ..ReconcilerOptions::new()
    })
    .unwrap();

    reconciler
        .expect(ExpectedPayment {
            memo_raw: memo_raw.clone(),
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            to: "0xrecipient".to_string(),
            amount: 10_000_000, // 10 pathUSD (6 decimals)
            from: None,
            due_at: None,
            meta: Some([("invoice_id".to_string(), "INV-001".to_string())].into()),
        })
        .expect("no duplicate");

    // 4. Ingest observed on-chain events.
    let events = vec![
        // exact match
        PaymentEvent {
            chain_id: 42431,
            block_number: 6_504_870,
            tx_hash: "0xaaaa".to_string(),
            log_index: 0,
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            from: "0xsender".to_string(),
            to: "0xrecipient".to_string(),
            amount: 10_000_000,
            memo_raw: Some(memo_raw.clone()),
            memo: None,
            timestamp: Some(1_700_000_000),
        },
        // unknown memo (no matching expected)
        PaymentEvent {
            chain_id: 42431,
            block_number: 6_504_871,
            tx_hash: "0xbbbb".to_string(),
            log_index: 0,
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            from: "0xother".to_string(),
            to: "0xrecipient".to_string(),
            amount: 5_000_000,
            memo_raw: None,
            memo: None,
            timestamp: Some(1_700_000_001),
        },
    ];

    let results = reconciler.ingest_many(events);
    for r in &results {
        println!("status: {:?}", r.status);
    }

    // 5. Report and export.
    let report = reconciler.report();
    println!(
        "\nmatched: {}  issues: {}  pending: {}",
        report.summary.matched_count, report.summary.issue_count, report.summary.pending_count,
    );

    assert_eq!(report.summary.matched_count, 1);
    assert_eq!(results[1].status, MatchStatus::NoMemo);

    let csv = export_csv(&report.matched);
    println!("\nCSV (matched):\n{}", csv);

    let json = export_json(&report.issues);
    println!("JSON (issues):\n{}", json);
}
