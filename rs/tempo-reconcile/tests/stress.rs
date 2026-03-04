use tempo_reconcile::{
    encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params, ExpectedPayment, MatchStatus,
    MemoType, PaymentEvent, Reconciler, ReconcilerOptions,
};
use ulid::Ulid;

const TOKEN: &str = "0x20c0000000000000000000000000000000000000";
const TO: &str = "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

#[test]
#[ignore = "stress: run with -- --include-ignored"]
fn reconciler_handles_100k_events() {
    let tag = issuer_tag_from_namespace("stress-test");
    let mut r = Reconciler::new(ReconcilerOptions::new());

    const N: usize = 100_000;

    // Generate N unique memos using real ULIDs.
    let memos: Vec<String> = (0..N)
        .map(|_| {
            let ulid = Ulid::new().to_string();
            encode_memo_v1(&EncodeMemoV1Params {
                memo_type: MemoType::Invoice,
                issuer_tag: tag,
                ulid,
                salt: None,
            })
            .unwrap()
        })
        .collect();

    // Register N expected payments.
    for memo in &memos {
        r.expect(ExpectedPayment {
            memo_raw: memo.clone(),
            token: TOKEN.to_string(),
            to: TO.to_string(),
            amount: 1_000_000,
            from: None,
            due_at: None,
            meta: None,
        })
        .unwrap();
    }

    // Ingest N matching events.
    for (i, memo) in memos.iter().enumerate() {
        let result = r.ingest(PaymentEvent {
            chain_id: 42431,
            block_number: i as u64,
            log_index: 0,
            tx_hash: format!("0x{i:0>64x}"),
            token: TOKEN.to_string(),
            from: "0xsender".to_string(),
            to: TO.to_string(),
            amount: 1_000_000,
            memo_raw: Some(memo.clone()),
            memo: None,
            timestamp: None,
        });
        assert_eq!(result.status, MatchStatus::Matched);
    }

    let report = r.report();
    assert_eq!(report.matched.len(), N);
    assert_eq!(report.summary.total_received, N);
}
