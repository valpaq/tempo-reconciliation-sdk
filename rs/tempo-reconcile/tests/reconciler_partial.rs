mod reconciler_helpers;
use reconciler_helpers::*;
use tempo_reconcile::{MatchStatus, Reconciler, ReconcilerOptions, ToleranceMode};

// ── tolerance boundary ────────────────────────────────────────────────────

#[test]
fn tolerance_exact_boundary_matches() {
    // 100 bps = 1 % on 10_000_000 → tolerance = 100_000, min_acceptable = 9_900_000
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 9_900_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn tolerance_one_below_boundary_fails() {
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // 9_899_999 = min_acceptable − 1 → underpaid
    let result = r.ingest(make_event(Some(&memo), 9_899_999));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
}

#[test]
fn tolerance_100pct_allows_minimum_amount() {
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 10_000; // 100 %
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // Even 1 unit passes when tolerance is 100 %
    let result = r.ingest(make_event(Some(&memo), 1));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn tolerance_with_overpay_still_matches() {
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100; // 1 %
                                     // allow_overpayment = true by default
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 11_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.overpaid_by, Some(1_000_000));
}

// ── partial + tolerance ───────────────────────────────────────────────────

#[test]
fn partial_tolerance_cumulative_within_tolerance() {
    // 3 partial payments; cumulative = 9_950_000 ≥ min_acceptable (9_900_000 with 1 %)
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.amount_tolerance_bps = 100;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e1 = make_event(Some(&memo), 3_000_000);
    e1.log_index = 0;
    assert_eq!(r.ingest(e1).status, MatchStatus::Partial);

    let mut e2 = make_event(Some(&memo), 3_000_000);
    e2.log_index = 1;
    assert_eq!(r.ingest(e2).status, MatchStatus::Partial);

    // cumulative = 9_950_000 ≥ 9_900_000 → Matched
    let mut e3 = make_event(Some(&memo), 3_950_000);
    e3.log_index = 2;
    assert_eq!(r.ingest(e3).status, MatchStatus::Matched);
}

#[test]
fn partial_overpay_accumulation_reports_overpaid_by() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e1 = make_event(Some(&memo), 6_000_000);
    e1.log_index = 0;
    assert_eq!(r.ingest(e1).status, MatchStatus::Partial);

    // cumulative = 11_000_000 > 10_000_000 → Matched with overpaid_by = 1_000_000
    let mut e2 = make_event(Some(&memo), 5_000_000);
    e2.log_index = 1;
    let result = r.ingest(e2);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.overpaid_by, Some(1_000_000));
}

// ── expiry combos ─────────────────────────────────────────────────────────

#[test]
fn late_payment_tracked_when_reject_expired_false() {
    // reject_expired = false (default): late payment is still Matched, is_late = true
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.timestamp = Some(2_000_000); // after due_at
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, Some(true));
}

#[test]
fn on_time_payment_not_late() {
    let mut opts = ReconcilerOptions::new();
    opts.reject_expired = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(2_000_000);
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.timestamp = Some(1_000_000); // before due_at
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, Some(false));
}

#[test]
fn no_timestamp_is_late_is_none() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    // event has no timestamp → cannot compute is_late
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, None);
}

#[test]
fn no_due_at_is_late_is_none() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap(); // no due_at

    let mut event = make_event(Some(&memo), 10_000_000);
    event.timestamp = Some(9_999_999);
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, None);
}

#[test]
fn expired_partial_payment_rejected_when_reject_expired() {
    let mut opts = ReconcilerOptions::new();
    opts.reject_expired = true;
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 4_000_000); // partial amount
    event.timestamp = Some(2_000_000); // after due_at
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Expired);
    assert_eq!(result.is_late, Some(true));
}

#[test]
fn late_partial_accumulates_when_reject_expired_false() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    // reject_expired = false (default)
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    // Late partial payment — still accumulated since reject_expired=false
    let mut event = make_event(Some(&memo), 5_000_000);
    event.timestamp = Some(2_000_000);
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Partial);
    assert_eq!(result.is_late, Some(true));
}

// ── strict_sender combos ──────────────────────────────────────────────────

#[test]
fn strict_sender_true_no_expected_from_passes() {
    let mut opts = ReconcilerOptions::new();
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap(); // expected.from = None

    // Any sender is accepted when expected.from is not set
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn strict_sender_false_wrong_sender_ignored() {
    // strict_sender = false (default) → expected.from is never checked
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.from = Some("0xexpectedsender".to_string());
    r.expect(exp).unwrap();

    // event.from = "0xsender" differs, but strict_sender=false so no mismatch
    let result = r.ingest(make_event(Some(&memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn strict_sender_case_insensitive_match() {
    let mut opts = ReconcilerOptions::new();
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.from = Some("0xAABBCCDD".to_string()); // uppercase
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.from = "0xaabbccdd".to_string(); // lowercase
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── address case-insensitivity ────────────────────────────────────────────

#[test]
fn recipient_case_insensitive_match() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap(); // to = "0xrecipient"

    let mut event = make_event(Some(&memo), 10_000_000);
    event.to = "0xRECIPIENT".to_string(); // uppercase
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn token_case_insensitive_match() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut event = make_event(Some(&memo), 10_000_000);
    event.token = "0x20C0000000000000000000000000000000000000".to_string(); // uppercase
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── remove_expected clears partial state ─────────────────────────────────

#[test]
fn remove_expected_clears_partial_accumulation() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // Partial payment → partials[memo] = 5M
    let mut e1 = make_event(Some(&memo), 5_000_000);
    e1.log_index = 0;
    assert_eq!(r.ingest(e1).status, MatchStatus::Partial);

    // Remove clears both expected AND partial state
    r.remove_expected(&memo);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // New partial event (different log_index) → fresh start, not Matched
    let mut e2 = make_event(Some(&memo), 5_000_000);
    e2.log_index = 1;
    let result = r.ingest(e2);
    assert_eq!(result.status, MatchStatus::Partial);
    assert_eq!(result.remaining_amount, Some(5_000_000));
}

// ── non-v1 memos as expected keys ─────────────────────────────────────────

#[test]
fn text_memo_can_be_used_as_expected_key() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = "hello-payment-ref";
    r.expect(make_expected(memo, 10_000_000)).unwrap();

    let result = r.ingest(make_event(Some(memo), 10_000_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn unknown_text_memo_returns_unknown_memo_status() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    // Not registered in expected
    let result = r.ingest(make_event(Some("unregistered-ref"), 10_000_000));
    assert_eq!(result.status, MatchStatus::UnknownMemo);
}

// ── allow_partial = false: single payment checked independently ───────────

#[test]
fn partial_not_allowed_single_payment_underpaid() {
    // allow_partial=false (default): underpayment is MismatchAmount even if two payments sum correctly
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut e1 = make_event(Some(&memo), 6_000_000);
    e1.log_index = 0;
    let r1 = r.ingest(e1);
    assert_eq!(r1.status, MatchStatus::MismatchAmount);
    assert_eq!(r1.remaining_amount, Some(4_000_000));

    // Second payment also underpaid; no accumulation → still MismatchAmount
    let mut e2 = make_event(Some(&memo), 4_000_000);
    e2.log_index = 1;
    let r2 = r.ingest(e2);
    assert_eq!(r2.status, MatchStatus::MismatchAmount);
}

// ── combined: overpay forbidden + late ───────────────────────────────────

#[test]
fn overpay_forbidden_and_late_triggers_mismatch_amount() {
    let mut opts = ReconcilerOptions::new();
    opts.allow_overpayment = false;
    // reject_expired = false so expiry is not the cause
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.due_at = Some(1_000_000);
    r.expect(exp).unwrap();

    let mut event = make_event(Some(&memo), 11_000_000); // overpaid
    event.timestamp = Some(2_000_000); // late, but reject_expired=false
    let result = r.ingest(event);
    assert_eq!(result.status, MatchStatus::MismatchAmount);
    assert_eq!(result.overpaid_by, Some(1_000_000));
    assert_eq!(result.is_late, Some(true));
}

// ── ToleranceMode::Each ───────────────────────────────────────────────────

#[test]
fn each_mode_within_tolerance_matches_immediately() {
    // Each mode: tolerance applies per individual payment.
    // 9.6M for expected 10M: underpaid by 400K < tolerance 500K → matched immediately.
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.amount_tolerance_bps = 500; // 5% tolerance = 500K on 10M
    opts.partial_tolerance_mode = ToleranceMode::Each;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // 9_600_000 for expected 10_000_000: underpaid by 400_000 < tolerance 500_000 → matched immediately
    let result = r.ingest(make_event(Some(&memo), 9_600_000));
    assert_eq!(
        result.status,
        MatchStatus::Matched,
        "within tolerance in each-mode → matched immediately"
    );
}

#[test]
fn tolerance_mode_final_matches_below_full_amount() {
    // Final mode: cumulative only needs to reach min_acceptable (expected - tolerance).
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.amount_tolerance_bps = 1000; // 10% → min_acceptable = 9_000_000
    opts.partial_tolerance_mode = tempo_reconcile::ToleranceMode::Final;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // Two payments of 4_600_000 each → cumulative = 9_200_000 ≥ 9_000_000 → Matched.
    let mut e1 = make_event(Some(&memo), 4_600_000);
    e1.log_index = 0;
    assert_eq!(r.ingest(e1).status, MatchStatus::Partial);

    let mut e2 = make_event(Some(&memo), 4_600_000);
    e2.log_index = 1;
    assert_eq!(r.ingest(e2).status, MatchStatus::Matched);
}

// ── total_expected survives remove_expected ────────────────────────────────

#[test]
fn total_expected_count_stable_after_remove_expected() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo1 = make_memo(MemoType::Invoice, ULID_A);
    let memo2 = make_memo(MemoType::Payroll, ULID_A);
    r.expect(make_expected(&memo1, 10_000_000)).unwrap();
    r.expect(make_expected(&memo2, 5_000_000)).unwrap();

    // Cancel memo2 — total_expected must still reflect 2 original registrations.
    r.remove_expected(&memo2);

    let report = r.report();
    assert_eq!(
        report.summary.total_expected, 2,
        "cancelled expected still counted"
    );
    assert_eq!(report.summary.total_expected_amount, 15_000_000);
    assert_eq!(
        report.summary.pending_count, 1,
        "only memo1 remains pending"
    );
}

// ── idempotency with different event keys ─────────────────────────────────

#[test]
fn two_events_same_tx_hash_different_log_index_are_independent() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // First event matches
    let mut e1 = make_event(Some(&memo), 10_000_000);
    e1.log_index = 0;
    let r1 = r.ingest(e1);
    assert_eq!(r1.status, MatchStatus::Matched);

    // Second event with same tx_hash but different log_index — expected already removed
    let mut e2 = make_event(Some(&memo), 10_000_000);
    e2.log_index = 1;
    let r2 = r.ingest(e2);
    assert_eq!(r2.status, MatchStatus::UnknownMemo);
}

// ── edge cases ───────────────────────────────────────────────────────────

#[test]
fn zero_amount_event_is_mismatch_amount() {
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    let result = r.ingest(make_event(Some(&memo), 0));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
}

#[test]
fn each_mode_beyond_tolerance_is_mismatch_amount() {
    // ToleranceMode::Each: if underpaid by more than tolerance → MismatchAmount immediately.
    // 8M for expected 10M: underpaid by 2M > tolerance 500K → MismatchAmount.
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 500; // 5% tolerance = 500K on 10M
    opts.allow_partial = true;
    opts.partial_tolerance_mode = ToleranceMode::Each;
    let mut r = Reconciler::new(opts).unwrap();
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    // 8M for expected 10M: underpaid by 2M > tolerance 500K → mismatch_amount
    let result = r.ingest(make_event(Some(&memo), 8_000_000));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
}

// ── overflow safety ───────────────────────────────────────────────────────

#[test]
fn partial_accumulation_saturates_at_u128_max() {
    // Two partial payments each of (u128::MAX/2 + 1) would overflow with plain `+=`.
    // saturating_add must clamp to u128::MAX and not panic.
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Payroll, ULID_B);
    r.expect(make_expected(&memo, u128::MAX)).unwrap();

    let half_plus = u128::MAX / 2 + 1;

    let mut e1 = make_event(Some(&memo), half_plus);
    e1.log_index = 0;
    assert_eq!(r.ingest(e1).status, MatchStatus::Partial);

    // saturating_add: (MAX/2+1) + (MAX/2+1) saturates to MAX == expected amount → Matched
    let mut e2 = make_event(Some(&memo), half_plus);
    e2.log_index = 1;
    assert_eq!(r.ingest(e2).status, MatchStatus::Matched);
}

#[test]
fn tolerance_from_bps_hundred_percent_at_u128_max() {
    // With 100% tolerance, any payment ≥ 0 should match.
    // Old saturating_mul path: saturating_mul(u128::MAX, 10_000) = u128::MAX,
    // then ceiling_div(u128::MAX, 10_000) ≈ u128::MAX/10_000 (only ~0.01% tolerance).
    // That makes min_acceptable ≈ 0.9999 * u128::MAX, rejecting a payment of 1.
    // New tolerance_from_bps correctly computes tolerance = u128::MAX → min_acceptable = 0.
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 10_000; // 100%
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, u128::MAX)).unwrap();

    // Payment of 1 must match: 100% tolerance means any amount ≥ 0 is accepted.
    let result = r.ingest(make_event(Some(&memo), 1));
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── tolerance + strict_sender ─────────────────────────────────────────────

#[test]
fn tolerance_strict_sender_wrong_from_is_mismatch_party() {
    // Sender check runs before amount check. Even if the amount is within tolerance,
    // a wrong sender must yield MismatchParty.
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100; // 1%
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut expected = make_expected(&memo, 10_000_000);
    expected.from = Some("0xsender".to_string());
    r.expect(expected).unwrap();

    let mut event = make_event(Some(&memo), 9_900_000); // within 1% tolerance
    event.from = "0xwrongsender".to_string();
    assert_eq!(r.ingest(event).status, MatchStatus::MismatchParty);
}

#[test]
fn tolerance_strict_sender_correct_from_matches() {
    // Amount within tolerance AND sender matches → Matched.
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 100; // 1%
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut expected = make_expected(&memo, 10_000_000);
    expected.from = Some("0xsender".to_string()); // make_event sets from="0xsender"
    r.expect(expected).unwrap();

    let result = r.ingest(make_event(Some(&memo), 9_900_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── partial + expired ─────────────────────────────────────────────────────

#[test]
fn partial_then_expired_payment_stays_expired() {
    // First payment arrives on time → Partial.
    // Second payment arrives after due_at with reject_expired=true → Expired,
    // even though cumulative total would exceed the expected amount.
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.reject_expired = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut expected = make_expected(&memo, 10_000_000);
    expected.due_at = Some(1000);
    r.expect(expected).unwrap();

    let mut ev1 = make_event(Some(&memo), 3_000_000);
    ev1.timestamp = Some(500); // on time
    ev1.log_index = 0;
    assert_eq!(r.ingest(ev1).status, MatchStatus::Partial);

    // Late payment — reject_expired fires before partial accumulation.
    let mut ev2 = make_event(Some(&memo), 10_000_000);
    ev2.timestamp = Some(2000); // after due_at=1000
    ev2.log_index = 1;
    assert_eq!(r.ingest(ev2).status, MatchStatus::Expired);
}

// ── ingest_many ───────────────────────────────────────────────────────────

#[test]
fn ingest_many_processes_all_events_independently() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo_a = make_memo(MemoType::Invoice, ULID_A);
    let memo_b = make_memo(MemoType::Invoice, ULID_B);
    r.expect(make_expected(&memo_a, 1_000_000)).unwrap();
    r.expect(make_expected(&memo_b, 2_000_000)).unwrap();

    // ev1: matches memo_a; ev2: unknown memo; ev3: matches memo_b
    // Each event must have a unique (tx_hash, log_index) key to avoid idempotency cache hits.
    let mut ev1 = make_event(Some(&memo_a), 1_000_000);
    ev1.log_index = 0;
    let mut ev2 = make_event(
        Some("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"),
        500_000,
    );
    ev2.log_index = 1;
    let mut ev3 = make_event(Some(&memo_b), 2_000_000);
    ev3.log_index = 2;

    let results = r.ingest_many(vec![ev1, ev2, ev3]);
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].status, MatchStatus::Matched);
    assert_eq!(results[1].status, MatchStatus::UnknownMemo);
    assert_eq!(results[2].status, MatchStatus::Matched);
}

#[test]
fn ingest_many_returns_results_in_input_order() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo_a = make_memo(MemoType::Invoice, ULID_A);
    let memo_b = make_memo(MemoType::Invoice, ULID_B);
    r.expect(make_expected(&memo_a, 100)).unwrap();
    r.expect(make_expected(&memo_b, 200)).unwrap();

    let mut ev_a = make_event(Some(&memo_a), 100);
    ev_a.log_index = 0;
    let mut ev_b = make_event(Some(&memo_b), 200);
    ev_b.log_index = 1;

    let results = r.ingest_many(vec![ev_a.clone(), ev_b.clone()]);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].payment.log_index, ev_a.log_index);
    assert_eq!(results[1].payment.log_index, ev_b.log_index);
}

// ── is_late edge cases ────────────────────────────────────────────────────

#[test]
fn is_late_none_when_event_has_no_timestamp_with_due_at() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 1_000_000);
    exp.due_at = Some(1_000);
    r.expect(exp).unwrap();

    let mut ev = make_event(Some(&memo), 1_000_000);
    ev.timestamp = None; // no timestamp → can't determine lateness
    let result = r.ingest(ev);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, None);
}

#[test]
fn is_late_none_when_expected_has_no_due_at_with_timestamp() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 1_000_000);
    exp.due_at = None; // no deadline
    r.expect(exp).unwrap();

    let mut ev = make_event(Some(&memo), 1_000_000);
    ev.timestamp = Some(2_000);
    let result = r.ingest(ev);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, None);
}

#[test]
fn is_late_false_when_payment_arrives_exactly_on_due_at() {
    let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 1_000_000);
    exp.due_at = Some(1_000);
    r.expect(exp).unwrap();

    let mut ev = make_event(Some(&memo), 1_000_000);
    ev.timestamp = Some(1_000); // exactly on due_at, not strictly after
    let result = r.ingest(ev);
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.is_late, Some(false));
}

#[test]
fn tolerance_mode_each_with_allow_overpayment_false() {
    // Underpayment within tolerance → Matched (tolerance still works)
    {
        let mut opts = ReconcilerOptions::new();
        opts.allow_overpayment = false;
        opts.amount_tolerance_bps = 100; // 1 %
        opts.partial_tolerance_mode = ToleranceMode::Each;
        let mut r = Reconciler::new(opts).unwrap();
        let memo = make_memo(MemoType::Invoice, ULID_A);
        r.expect(make_expected(&memo, 10_000_000)).unwrap();
        // 9_900_000 = exactly at the 1 % boundary — must match
        let result = r.ingest(make_event(Some(&memo), 9_900_000));
        assert_eq!(result.status, MatchStatus::Matched);
    }

    // Overpayment → MismatchAmount because allow_overpayment = false
    {
        let mut opts = ReconcilerOptions::new();
        opts.allow_overpayment = false;
        opts.amount_tolerance_bps = 100;
        opts.partial_tolerance_mode = ToleranceMode::Each;
        let mut r = Reconciler::new(opts).unwrap();
        let memo = make_memo(MemoType::Invoice, ULID_B);
        r.expect(make_expected(&memo, 10_000_000)).unwrap();
        let result = r.ingest(make_event(Some(&memo), 10_000_001));
        assert_eq!(result.status, MatchStatus::MismatchAmount);
    }
}

#[test]
fn each_mode_does_not_accumulate_partials() {
    // Verify that each-mode within-tolerance match doesn't leave partial state
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 200; // 2%
    opts.allow_partial = true;
    opts.partial_tolerance_mode = ToleranceMode::Each;
    let mut r = Reconciler::new(opts).unwrap();
    r.expect(make_expected(&memo, 10_000_000)).unwrap();
    let result = r.ingest(make_event(Some(&memo), 9_850_000));
    assert_eq!(result.status, MatchStatus::Matched);
    // After match, expected should be removed
    assert_eq!(r.pending_count(), 0);
}

// ── partial state preserved after mismatch ───────────────────────────────

#[test]
fn partial_then_wrong_token_preserves_partial_state() {
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::new(ReconcilerOptions {
        allow_partial: true,
        ..ReconcilerOptions::new()
    })
    .unwrap();
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    // First partial: correct token, 4M
    let mut ev1 = make_event(Some(&memo), 4_000_000);
    ev1.log_index = 0;
    let r1 = r.ingest(ev1);
    assert_eq!(r1.status, MatchStatus::Partial);
    assert_eq!(r1.remaining_amount, Some(6_000_000));

    // Second payment: wrong token → MismatchToken
    let mut ev2 = make_event(Some(&memo), 6_000_000);
    ev2.log_index = 1;
    ev2.token = "0xwrongtoken0000000000000000000000000000".to_string();
    let r2 = r.ingest(ev2);
    assert_eq!(r2.status, MatchStatus::MismatchToken);

    // Third payment: correct token, 6M → should complete the match
    // (partial state from ev1 was preserved, not lost)
    let mut ev3 = make_event(Some(&memo), 6_000_000);
    ev3.log_index = 2;
    let r3 = r.ingest(ev3);
    assert_eq!(r3.status, MatchStatus::Matched);
}

#[test]
fn partial_then_wrong_recipient_preserves_partial_state() {
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::new(ReconcilerOptions {
        allow_partial: true,
        ..ReconcilerOptions::new()
    })
    .unwrap();
    r.expect(make_expected(&memo, 10_000_000)).unwrap();

    let mut ev1 = make_event(Some(&memo), 4_000_000);
    ev1.log_index = 0;
    assert_eq!(r.ingest(ev1).status, MatchStatus::Partial);

    // Wrong recipient
    let mut ev2 = make_event(Some(&memo), 6_000_000);
    ev2.log_index = 1;
    ev2.to = "0xwrongrecipient00000000000000000000000000".to_string();
    assert_eq!(r.ingest(ev2).status, MatchStatus::MismatchParty);

    // Correct recipient completes
    let mut ev3 = make_event(Some(&memo), 6_000_000);
    ev3.log_index = 2;
    assert_eq!(r.ingest(ev3).status, MatchStatus::Matched);
}

// ── partial + strict_sender interaction ───────────────────────────────────

#[test]
fn partial_with_strict_sender_correct_sender_accumulates() {
    // strict_sender + allow_partial: correct sender → partial accumulates
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.from = Some("0xsender".to_string()); // matches make_event default
    r.expect(exp).unwrap();

    let mut ev1 = make_event(Some(&memo), 4_000_000);
    ev1.log_index = 0;
    assert_eq!(r.ingest(ev1).status, MatchStatus::Partial);

    let mut ev2 = make_event(Some(&memo), 6_000_000);
    ev2.log_index = 1;
    assert_eq!(r.ingest(ev2).status, MatchStatus::Matched);
}

#[test]
fn partial_with_strict_sender_wrong_sender_rejected_but_state_preserved() {
    // strict_sender + allow_partial: wrong sender → MismatchParty,
    // but correct-sender partials still accumulate
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut exp = make_expected(&memo, 10_000_000);
    exp.from = Some("0xsender".to_string());
    r.expect(exp).unwrap();

    // Correct sender partial
    let mut ev1 = make_event(Some(&memo), 4_000_000);
    ev1.log_index = 0;
    assert_eq!(r.ingest(ev1).status, MatchStatus::Partial);

    // Wrong sender → rejected
    let mut ev2 = make_event(Some(&memo), 6_000_000);
    ev2.log_index = 1;
    ev2.from = "0xwrongsender".to_string();
    assert_eq!(r.ingest(ev2).status, MatchStatus::MismatchParty);

    // Correct sender completes the partial
    let mut ev3 = make_event(Some(&memo), 6_000_000);
    ev3.log_index = 2;
    assert_eq!(r.ingest(ev3).status, MatchStatus::Matched);
}

#[test]
fn partial_strict_sender_no_expected_from_allows_any_sender() {
    // strict_sender but expected.from = None → any sender accepted for partials
    let mut opts = ReconcilerOptions::new();
    opts.allow_partial = true;
    opts.strict_sender = true;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_000)).unwrap(); // from = None

    let mut ev1 = make_event(Some(&memo), 4_000_000);
    ev1.log_index = 0;
    ev1.from = "0xalice".to_string();
    assert_eq!(r.ingest(ev1).status, MatchStatus::Partial);

    let mut ev2 = make_event(Some(&memo), 6_000_000);
    ev2.log_index = 1;
    ev2.from = "0xbob".to_string(); // different sender, but expected.from is None
    assert_eq!(r.ingest(ev2).status, MatchStatus::Matched);
}

// ── tolerance rounding edge cases ────────────────────────────────────────

#[test]
fn tolerance_rounding_non_integer_division_rounds_up() {
    // 333 bps on 10_000_001 → tolerance = ceil(10_000_001 * 333 / 10_000)
    // 10_000_001 * 333 / 10_000 = 333_000.0333... → ceil = 333_001
    // min_acceptable = 10_000_001 - 333_001 = 9_667_000
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 333;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_001)).unwrap();

    // Exactly at min_acceptable → should match
    let result = r.ingest(make_event(Some(&memo), 9_667_000));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn tolerance_rounding_one_below_ceil_boundary_fails() {
    // Same as above but 1 unit below min_acceptable
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 333;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 10_000_001)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 9_666_999));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
}

#[test]
fn tolerance_1_bps_on_small_amount_rounds_up_to_1() {
    // 1 bps on amount=100: tolerance = ceil(100 * 1 / 10_000) = ceil(0.01) = 1
    // min_acceptable = 99
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 1;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 100)).unwrap();

    // 99 should match (min_acceptable = 99)
    let result = r.ingest(make_event(Some(&memo), 99));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn tolerance_1_bps_on_small_amount_98_fails() {
    // 1 bps on amount=100: tolerance = 1, min_acceptable = 99
    // 98 < 99 → mismatch
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 1;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_B);
    r.expect(make_expected(&memo, 100)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 98));
    assert_eq!(result.status, MatchStatus::MismatchAmount);
}

#[test]
fn tolerance_on_amount_1_yields_tolerance_1_at_10000bps() {
    // 10000 bps (100%) on amount=1 → tolerance = 1, min = 0
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 10_000;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_A);
    r.expect(make_expected(&memo, 1)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 0));
    assert_eq!(result.status, MatchStatus::Matched);
}

#[test]
fn tolerance_on_amount_1_at_5000bps_rounds_up_to_1() {
    // 5000 bps (50%) on amount=1 → tolerance = ceil(1 * 5000 / 10_000) = ceil(0.5) = 1
    // min_acceptable = 0
    let mut opts = ReconcilerOptions::new();
    opts.amount_tolerance_bps = 5_000;
    let mut r = Reconciler::new(opts).unwrap();

    let memo = make_memo(MemoType::Invoice, ULID_B);
    r.expect(make_expected(&memo, 1)).unwrap();

    let result = r.ingest(make_event(Some(&memo), 0));
    assert_eq!(result.status, MatchStatus::Matched);
}

// ── expected amount = 0 with tolerance ───────────────────────────────────

#[test]
fn expected_amount_zero_with_tolerance_returns_matched() {
    // expected.amount = 0, tolerance = 10% (1000 bps)
    // tolerance_from_bps(0, 1000) = 0, so min_acceptable = 0
    // event.amount = 0 should match exactly
    let memo = make_memo(MemoType::Invoice, ULID_A);
    let mut r = Reconciler::new(ReconcilerOptions {
        amount_tolerance_bps: 1000,
        ..ReconcilerOptions::new()
    })
    .unwrap();
    r.expect(make_expected(&memo, 0)).unwrap();
    let result = r.ingest(make_event(Some(&memo), 0));
    assert_eq!(result.status, MatchStatus::Matched);
}
