mod reconciler_helpers;
use reconciler_helpers::*;
use tempo_reconcile::{
    ExpectedPayment, InMemoryStore, MatchResult, MatchStatus, PaymentEvent, ReconcileError,
    ReconcileStore, Reconciler, ReconcilerOptions,
};

// ── ingest_many ───────────────────────────────────────────────────────────

#[test]
fn ingest_many_processes_all_events() {
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memo1 = make_memo(MemoType::Invoice, ULID_A);
    let memo2 = make_memo(MemoType::Payroll, ULID_A); // same ULID, different type byte

    r.expect(make_expected(&memo1, 10_000_000)).unwrap();
    r.expect(make_expected(&memo2, 5_000_000)).unwrap();

    let mut e1 = make_event(Some(&memo1), 10_000_000);
    e1.log_index = 0;
    let mut e2 = make_event(Some(&memo2), 5_000_000);
    e2.log_index = 1;

    let results = r.ingest_many(vec![e1, e2]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].status, MatchStatus::Matched);
    assert_eq!(results[1].status, MatchStatus::Matched);
}

#[test]
fn ingest_many_preserves_result_order() {
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e_matched = make_event(Some(&memo), 10_000_000);
    e_matched.log_index = 0;
    let mut e_no_memo = make_event(None, 500);
    e_no_memo.log_index = 1;

    let results = r.ingest_many(vec![e_matched, e_no_memo]);
    assert_eq!(results[0].status, MatchStatus::Matched);
    assert_eq!(results[1].status, MatchStatus::NoMemo);
}

// ── report totals ─────────────────────────────────────────────────────────

#[test]
fn report_total_received_amount_sums_all_events() {
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e1 = make_event(Some(&memo), 10_000_000);
    e1.log_index = 0;
    let mut e2 = make_event(None, 500_000); // no_memo
    e2.log_index = 1;

    r.ingest(e1);
    r.ingest(e2);

    let report = r.report();
    assert_eq!(report.summary.total_received_amount, 10_500_000);
}

#[test]
fn report_total_expected_amount_includes_pending_and_matched() {
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memo1 = make_memo(MemoType::Invoice, ULID_A);
    let memo2 = make_memo(MemoType::Payroll, ULID_A); // same ULID, different type byte
    r.expect(make_expected(&memo1, 10_000_000)).unwrap();
    r.expect(make_expected(&memo2, 5_000_000)).unwrap();

    r.ingest(make_event(Some(&memo1), 10_000_000));
    // memo2 left pending

    let report = r.report();
    assert_eq!(report.summary.total_expected_amount, 15_000_000);
    assert_eq!(report.summary.total_matched_amount, 10_000_000);
}

#[test]
fn report_issue_count_equals_issues_len() {
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e1 = make_event(None, 100); // no_memo
    e1.log_index = 0;
    let mut e2 = make_event(Some(&memo), 1_000); // mismatch_amount
    e2.log_index = 1;
    let e3_memo = make_memo(MemoType::Invoice, ULID_B);
    let mut e3 = make_event(Some(&e3_memo), 100); // unknown_memo
    e3.log_index = 2;

    r.ingest(e1);
    r.ingest(e2);
    r.ingest(e3);

    let report = r.report();
    assert_eq!(report.summary.issue_count, report.issues.len());
    assert_eq!(report.issues.len(), 3);
}

// ── all memo types ────────────────────────────────────────────────────────

#[test]
fn all_memo_types_can_be_matched() {
    use MemoType::*;
    // All types with the same ULID — different type byte makes each memo_raw unique
    let type_ulid_pairs: &[(MemoType, &str)] = &[
        (Invoice, ULID_A),
        (Payroll, ULID_A),
        (Refund, ULID_A),
        (Batch, ULID_A),
        (Subscription, ULID_A),
        (Custom, ULID_A),
    ];

    let mut r = Reconciler::new(ReconcilerOptions::new());
    let memos: Vec<String> = type_ulid_pairs
        .iter()
        .map(|(t, u)| make_memo(t.clone(), u))
        .collect();

    for (i, memo) in memos.iter().enumerate() {
        r.expect(make_expected(memo, 1_000_000)).unwrap();
        let mut event = make_event(Some(memo), 1_000_000);
        event.log_index = i as u32;
        let result = r.ingest(event);
        assert_eq!(
            result.status,
            MatchStatus::Matched,
            "memo type index {} did not match",
            i
        );
    }
}

#[tokio::test]
async fn concurrent_ingest_via_arc_mutex() {
    use std::sync::Arc;

    // 10 distinct memos — ULIDs are zero-padded to 26 Crockford base32 chars.
    let memos: Vec<String> = (0..10u32)
        .map(|i| {
            let ulid = format!("{i:0>26}"); // e.g. "00000000000000000000000001"
            make_memo(MemoType::Invoice, &ulid)
        })
        .collect();

    let mut r = Reconciler::new(ReconcilerOptions::new());
    for memo in &memos {
        let exp = ExpectedPayment {
            memo_raw: memo.clone(),
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            to: "0xrecipient".to_string(),
            amount: 1_000_000,
            from: None,
            due_at: None,
            meta: None,
        };
        r.expect(exp).unwrap();
    }

    let shared = Arc::new(tokio::sync::Mutex::new(r));

    let tasks: Vec<_> = memos
        .into_iter()
        .enumerate()
        .map(|(i, memo)| {
            let shared = shared.clone();
            tokio::spawn(async move {
                let event = PaymentEvent {
                    chain_id: 42431,
                    block_number: i as u64,
                    tx_hash: format!("0x{i:0>64}"),
                    log_index: 0,
                    token: "0x20c0000000000000000000000000000000000000".to_string(),
                    from: "0xsender".to_string(),
                    to: "0xrecipient".to_string(),
                    amount: 1_000_000,
                    memo_raw: Some(memo),
                    memo: None,
                    timestamp: None,
                };
                let mut guard = shared.lock().await;
                guard.ingest(event)
            })
        })
        .collect();

    for task in tasks {
        task.await.unwrap();
    }

    let report = shared.lock().await.report();
    assert_eq!(report.summary.total_received, 10);
    assert_eq!(report.matched.len(), 10);
}

// ── meta field preservation ────────────────────────────────────────────────

#[test]
fn meta_field_preserved_in_matched_result() {
    use std::collections::HashMap;
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::new(ReconcilerOptions::new());
    let mut exp = make_expected(&memo, 10_000_000);
    exp.meta = Some(HashMap::from([
        ("invoiceId".to_string(), "INV-001".to_string()),
        ("customer".to_string(), "Acme Corp".to_string()),
    ]));
    r.expect(exp).unwrap();
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
    let meta = result.expected.unwrap().meta.unwrap();
    assert_eq!(meta["invoiceId"], "INV-001");
    assert_eq!(meta["customer"], "Acme Corp");
}

// ── ceiling_div overflow safety ───────────────────────────────────────────

#[test]
fn ceiling_div_no_overflow_at_u128_max() {
    // Tolerance calculation uses ceiling_div internally.
    // With a = u128::MAX and b = 10_000, (a + b - 1) would overflow.
    // Verify the safe formula handles this correctly.
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 1; // 0.01% — non-zero to exercise ceiling_div
    let mut r = Reconciler::new(opts);
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, u128::MAX)).unwrap();
    let result = r.ingest(make_event(Some(&memo), u128::MAX));
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── custom ReconcileStore via with_store ──────────────────────────────────

#[test]
fn custom_store_works_with_reconciler() {
    struct MyStore(InMemoryStore);

    impl ReconcileStore for MyStore {
        fn add_expected(&mut self, payment: ExpectedPayment) -> Result<(), ReconcileError> {
            self.0.add_expected(payment)
        }
        fn get_expected(&self, memo_raw: &str) -> Option<&ExpectedPayment> {
            self.0.get_expected(memo_raw)
        }
        fn get_all_expected(&self) -> Vec<&ExpectedPayment> {
            self.0.get_all_expected()
        }
        fn remove_expected(&mut self, memo_raw: &str) -> bool {
            self.0.remove_expected(memo_raw)
        }
        fn add_result(&mut self, key: &str, result: MatchResult) {
            self.0.add_result(key, result)
        }
        fn get_result(&self, key: &str) -> Option<&MatchResult> {
            self.0.get_result(key)
        }
        fn get_all_results(&self) -> Vec<&MatchResult> {
            self.0.get_all_results()
        }
        fn add_partial(&mut self, memo_raw: &str, amount: u128) -> u128 {
            self.0.add_partial(memo_raw, amount)
        }
        fn get_partial_total(&self, memo_raw: &str) -> u128 {
            self.0.get_partial_total(memo_raw)
        }
        fn remove_partial(&mut self, memo_raw: &str) {
            self.0.remove_partial(memo_raw)
        }
        fn clear(&mut self) {
            self.0.clear()
        }
    }

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::with_store(MyStore(InMemoryStore::new()), ReconcilerOptions::new());
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── all 8 match statuses in one report ────────────────────────────────────

#[test]
fn report_contains_all_eight_match_statuses() {
    use std::collections::HashSet;
    use MemoType::*;

    // Options that enable all status paths in one reconciler
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true; // enables Partial
    opts.reject_expired = true; // enables Expired
    opts.allow_overpayment = false; // enables MismatchAmount via overpayment

    let mut r = Reconciler::new(opts);

    // Six unique memos via different type codes
    let m_matched = make_memo(Invoice, ULID_A);
    let m_partial = make_memo(Payroll, ULID_A);
    let m_mismatch = make_memo(Refund, ULID_A);
    let m_tok = make_memo(Batch, ULID_A);
    let m_party = make_memo(Subscription, ULID_A);
    let m_expired = make_memo(Custom, ULID_A);

    r.expect(make_expected(&m_matched, 10_000_000)).unwrap();
    r.expect(make_expected(&m_partial, 10_000_000)).unwrap();
    r.expect(make_expected(&m_mismatch, 10_000_000)).unwrap();
    r.expect(make_expected(&m_tok, 10_000_000)).unwrap();
    r.expect(make_expected(&m_party, 10_000_000)).unwrap();
    let mut exp_expired = make_expected(&m_expired, 10_000_000);
    exp_expired.due_at = Some(1_000);
    r.expect(exp_expired).unwrap();

    // 1. Matched
    let mut e = make_event(Some(&m_matched), 10_000_000);
    e.log_index = 0;
    r.ingest(e);

    // 2. Partial (underpayment, allow_partial=true)
    let mut e = make_event(Some(&m_partial), 3_000_000);
    e.log_index = 1;
    r.ingest(e);

    // 3. MismatchAmount (overpayment, allow_overpayment=false)
    let mut e = make_event(Some(&m_mismatch), 15_000_000);
    e.log_index = 2;
    r.ingest(e);

    // 4. MismatchToken
    let mut e = make_event(Some(&m_tok), 10_000_000);
    e.token = "0xwrongtoken".to_string();
    e.log_index = 3;
    r.ingest(e);

    // 5. MismatchParty
    let mut e = make_event(Some(&m_party), 10_000_000);
    e.to = "0xwrongrecipient".to_string();
    e.log_index = 4;
    r.ingest(e);

    // 6. Expired
    let mut e = make_event(Some(&m_expired), 10_000_000);
    e.timestamp = Some(999_999_999);
    e.log_index = 5;
    r.ingest(e);

    // 7. UnknownMemo
    let m_unknown = make_memo(Invoice, ULID_B);
    let mut e = make_event(Some(&m_unknown), 10_000_000);
    e.log_index = 6;
    r.ingest(e);

    // 8. NoMemo
    let mut e = make_event(None, 1_000_000);
    e.log_index = 7;
    r.ingest(e);

    let report = r.report();
    let all_statuses: HashSet<String> = report
        .matched
        .iter()
        .chain(report.issues.iter())
        .map(|r| r.status.as_str().to_string())
        .collect();

    for expected_status in [
        "matched",
        "partial",
        "unknown_memo",
        "no_memo",
        "mismatch_amount",
        "mismatch_token",
        "mismatch_party",
        "expired",
    ] {
        assert!(
            all_statuses.contains(expected_status),
            "missing status: {expected_status}"
        );
    }
}
