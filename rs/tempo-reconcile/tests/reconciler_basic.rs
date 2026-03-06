mod reconciler_helpers;
use reconciler_helpers::*;
use tempo_reconcile::{MatchStatus, ReconcileError, Reconciler, ReconcilerOptions};

#[test]
fn matched_exact_amount() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
    assert!(result.overpaid_by.is_none());
}

#[test]
fn matched_with_overpayment() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap(); // allow_overpayment=true by default
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 11_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.overpaid_by, Some(1_000_000));
}

#[test]
fn no_memo() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let result = r.ingest(make_event(None, 10_000_000));
    assert_eq!(result.status, MatchStatus::NoMemo);
}

#[test]
fn unknown_memo() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    // not registered in expected
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::UnknownMemo);
}

#[test]
fn mismatch_amount_underpaid() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 5_000_000));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
    assert_eq!(result.remaining_amount, Some(5_000_000));
}

#[test]
fn mismatch_token() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.token = "0xwrongtoken".to_string();
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::MismatchToken);
}

#[test]
fn mismatch_party_recipient() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.to = "0xwrongrecipient".to_string();
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::MismatchParty);
}

#[test]
fn mismatch_party_sender_strict() {
    let mut opts = ReconcilerOptions::new();
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    let mut exp = make_expected(&memo, 10_000_000);
    exp.from = Some("0xexpectedsender".to_string());
    r.expect(exp).unwrap();

    let result = r.ingest(make_event(Some(&memo), 10_000_000)); // from = "0xsender"
    assert_eq!(result.status, MatchStatus::MismatchParty);
}

#[test]
fn expired_payment() {
    let mut opts = ReconcilerOptions::new();
    opts.reject_expired = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.timestamp = Some(2_000_000); // after due_at
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Expired);
    assert_eq!(result.is_late, Some(true));
}

#[test]
fn partial_payments_accumulate() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // First partial: 6M of 10M
    let mut e1 = make_event(Some(&memo), 6_000_000);
    e1.log_index = 0;
    let r1 = r.ingest(e1);
    assert_eq!(r1.status, MatchStatus::Partial);
    assert_eq!(r1.remaining_amount, Some(4_000_000));

    // Second partial: completes (4M more)
    let mut e2 = make_event(Some(&memo), 4_000_000);
    e2.log_index = 1;
    let r2 = r.ingest(e2);
    assert_eq!(r2.status, MatchStatus::Matched);
}

#[test]
fn idempotency_same_event() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let event = make_event(Some(&memo), 10_000_000);
    let r1 = r.ingest(event.clone());
    let r2 = r.ingest(event);
    assert_eq!(r1.status, r2.status);
}

#[test]
fn duplicate_expect_errors() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    assert!(r.expect(make_expected(&memo, 10_000_000)).is_err());
}

#[test]
fn report_summary() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    r.ingest(make_event(Some(&memo), 10_000_000));

    let report = r.report();
    assert_eq!(report.summary.matched_count, 1);
    assert_eq!(report.summary.pending_count, 0);
    assert!(report.matched.len() == 1);
}

#[test]
fn tolerance_allows_underpayment_within_bps() {
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100; // 1%
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // 9_900_000 = 10_000_000 - 1% → within tolerance
    let result = r.ingest(make_event(Some(&memo), 9_900_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn overpayment_rejected_when_disabled() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_overpayment = false;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 11_000_000));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
    assert_eq!(result.overpaid_by, Some(1_000_000));
}

#[test]
fn reset_clears_all_state() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    r.ingest(make_event(Some(&memo), 10_000_000));

    r.reset();

    let report = r.report();
    assert_eq!(report.summary.total_received, 0);
    assert_eq!(report.summary.pending_count, 0);
    assert_eq!(report.summary.matched_count, 0);
}

#[test]
fn remove_expected_prevents_match() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    assert!(r.remove_expected(&memo));
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::UnknownMemo);
}

#[test]
fn remove_expected_returns_false_when_not_found() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    assert!(!r.remove_expected("0xnonexistent"));
}

#[test]
fn re_register_after_remove() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    r.remove_expected(&memo);

    assert!(r.expect(make_expected(&memo, 10_000_000)).is_ok());
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn report_with_pending_and_issues() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo1 = make_memo(MemoType::Invoice, "01MASW9NF6YW40J40H289H858P");
    let memo2 = make_memo(MemoType::Payroll, "01MASW9NF6YW40J40H289H8580");

    r.expect(make_expected(&memo1, 10_000_000)).unwrap();
    r.expect(make_expected(&memo2, 5_000_000)).unwrap();

    r.ingest(make_event(Some(&memo1), 10_000_000));
    // Different log_index to avoid idempotency dedup
    let mut no_memo_event = make_event(None, 1_000_000);
    no_memo_event.log_index = 1;
    r.ingest(no_memo_event);

    let report = r.report();
    assert_eq!(report.summary.matched_count, 1);
    assert_eq!(report.summary.pending_count, 1);
    assert_eq!(report.summary.no_memo_count, 1);
    assert_eq!(report.matched.len(), 1);
    assert_eq!(report.pending.len(), 1);
    assert_eq!(report.issues.len(), 1);
}

// ── issuer_tag filter ─────────────────────────────────────────────────────

#[test]
fn issuer_tag_filter_allows_matching_tag() {
    let ns_tag = issuer_tag_from_namespace("test-ns");
    let mut opts = ReconcilerOptions::new();
    opts.issuer_tag = Some(ns_tag);
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn issuer_tag_filter_rejects_wrong_tag() {
    let mut opts = ReconcilerOptions::new();
    opts.issuer_tag = Some(0xdeadbeef_u64); // does not match test-ns
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A); // encoded with issuer_tag_from_namespace("test-ns")
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::UnknownMemo);
}

#[test]
fn issuer_tag_filter_rejects_non_v1_memo() {
    let mut opts = ReconcilerOptions::new();
    opts.issuer_tag = Some(issuer_tag_from_namespace("test-ns"));
    let mut r = Reconciler::new(opts).unwrap();

    // All-zero memo: type byte 0x00 is not a valid v1 code → decode returns None
    let memo = "0x0000000000000000000000000000000000000000000000000000000000000000";
    let result = r.ingest(make_event(Some(memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::UnknownMemo);
}

#[test]
fn concurrent_reconciler_with_mutex() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    // Register 100 expected payments
    let memos: Vec<String> = (0..100)
        .map(|i| {
            let ulid = format!("0{:025}", i); // 26 chars, all digits (valid Crockford)
            make_memo(MemoType::Invoice, &ulid)
        })
        .collect();
    for memo in &memos {
        r.expect(make_expected(memo, 1_000_000)).unwrap();
    }

    let r = Arc::new(Mutex::new(r));
    let memos = Arc::new(memos);

    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let r = Arc::clone(&r);
            let memos = Arc::clone(&memos);
            thread::spawn(move || {
                for i in 0..10 {
                    let idx = thread_id * 10 + i;
                    let memo = &memos[idx];
                    let mut event = make_event(Some(memo), 1_000_000);
                    event.tx_hash = format!("0x{:064x}", idx);
                    event.log_index = 0;
                    let result = r.lock().unwrap().ingest(event);
                    assert_eq!(result.status, MatchStatus::Matched);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let report = r.lock().unwrap().report();
    assert_eq!(report.summary.matched_count, 100);
    assert_eq!(report.summary.pending_count, 0);
}

#[test]
fn report_after_reset_is_empty() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    r.ingest(make_event(Some(&memo), 10_000_000));

    assert_eq!(r.expected_count(), 1);
    assert_eq!(r.result_count(), 1);

    r.reset();

    let report = r.report();
    assert_eq!(report.summary.total_expected, 0);
    assert_eq!(report.summary.total_received, 0);
    assert_eq!(report.summary.matched_count, 0);
    assert_eq!(report.summary.pending_count, 0);
    assert_eq!(report.matched.len(), 0);
    assert_eq!(report.issues.len(), 0);
    assert_eq!(report.pending.len(), 0);
    assert_eq!(r.expected_count(), 0);
    assert_eq!(r.result_count(), 0);
}

#[test]
fn remove_expected_clears_partial_state() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // Partial payment
    let mut e1 = make_event(Some(&memo), 5_000_000);
    e1.log_index = 0;
    let r1 = r.ingest(e1);
    assert_eq!(r1.status, MatchStatus::Partial);

    // Remove expected (should also clear partial accumulation)
    assert!(r.remove_expected(&memo));

    // Re-register with same memo
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // New partial should start from 0, not from the previous 5M
    let mut e2 = make_event(Some(&memo), 5_000_000);
    e2.tx_hash = "0xnew".to_string();
    e2.log_index = 0;
    let r2 = r.ingest(e2);
    assert_eq!(r2.status, MatchStatus::Partial);
    assert_eq!(r2.remaining_amount, Some(5_000_000)); // 5M remaining, not 0
}

#[test]
fn ingest_many_deduplicates_within_batch() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let event = make_event(Some(&memo), 10_000_000);
    // Same event 3 times in one batch
    let results = r.ingest_many(vec![event.clone(), event.clone(), event]);
    assert_eq!(results.len(), 3);
    // All should return Matched (first matches, rest return cached)
    assert_eq!(results[0].status, MatchStatus::Matched);
    assert_eq!(results[1].status, MatchStatus::Matched);
    assert_eq!(results[2].status, MatchStatus::Matched);
    // Only one result in cache
    assert_eq!(r.result_count(), 1);
}

#[test]
fn ingest_many_empty_returns_empty() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let results = r.ingest_many(vec![]);
    assert!(results.is_empty());
}

#[test]
fn options_returns_configured_values() {
    let opts = ReconcilerOptions {
        amount_tolerance_bps: 500,
        strict_sender: true,
        allow_partial: true,
        reject_expired: true,
        allow_overpayment: false,
        ..ReconcilerOptions::new()
    };
    let r = Reconciler::new(opts).unwrap();
    let o = r.options();
    assert_eq!(o.amount_tolerance_bps, 500);
    assert!(o.strict_sender);
    assert!(o.allow_partial);
    assert!(o.reject_expired);
    assert!(!o.allow_overpayment);
}

#[test]
fn rejects_bps_above_10000() {
    match Reconciler::new(ReconcilerOptions {
        amount_tolerance_bps: 20000,
        ..ReconcilerOptions::new()
    }) {
        Err(ReconcileError::InvalidToleranceBps(20000)) => {}
        Err(e) => panic!("unexpected error: {e}"),
        Ok(_) => panic!("expected InvalidToleranceBps error"),
    }
}

#[test]
fn report_total_matched_amount_uses_expected_amount() {
    // When overpayment is allowed: expected=100, payment=120 → totalMatchedAmount should be 100
    let mut opts = ReconcilerOptions::new();
    opts.allow_overpayment = true;
    let mut r = Reconciler::new(opts).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 100)).unwrap();
    let mut event = make_event(Some(&memo), 120);
    event.log_index = 0;
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
    let report = r.report();
    assert_eq!(report.summary.total_matched_amount, 100); // expected amount, not payment amount
    assert_eq!(report.summary.total_received_amount, 120); // received is still full payment
}
