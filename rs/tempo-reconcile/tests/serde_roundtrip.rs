#![cfg(feature = "serde")]

use std::collections::HashMap;
use tempo_reconcile::{
    ExpectedPayment, MatchResult, MatchStatus, Memo, MemoType, MemoV1, PaymentEvent,
    ReconcileReport, ReconcileSummary,
};

// ── helpers ───────────────────────────────────────────────────────────────

/// Roundtrip via JSON for types that implement PartialEq.
fn roundtrip<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + PartialEq,
{
    let json = serde_json::to_string(value).expect("serialize failed");
    serde_json::from_str::<T>(&json).expect("deserialize failed")
}

/// Roundtrip check via serde_json::Value for types that do not implement PartialEq.
/// Asserts that the re-serialized value equals the original JSON value.
fn roundtrip_via_value<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let json_str = serde_json::to_string(value).expect("serialize failed");
    let orig_val: serde_json::Value =
        serde_json::from_str(&json_str).expect("parse original json failed");
    let deserialized = serde_json::from_str::<T>(&json_str).expect("deserialize failed");
    let reserialised: serde_json::Value =
        serde_json::to_value(&deserialized).expect("re-serialize failed");
    assert_eq!(orig_val, reserialised, "roundtrip mismatch for {:?}", value);
}

// ── MemoType ──────────────────────────────────────────────────────────────

#[test]
fn memo_type_invoice_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Invoice), MemoType::Invoice);
}

#[test]
fn memo_type_payroll_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Payroll), MemoType::Payroll);
}

#[test]
fn memo_type_refund_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Refund), MemoType::Refund);
}

#[test]
fn memo_type_batch_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Batch), MemoType::Batch);
}

#[test]
fn memo_type_subscription_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Subscription), MemoType::Subscription);
}

#[test]
fn memo_type_custom_roundtrip() {
    assert_eq!(roundtrip(&MemoType::Custom), MemoType::Custom);
}

#[test]
fn memo_type_uses_snake_case_serialization() {
    assert_eq!(
        serde_json::to_string(&MemoType::Invoice).unwrap(),
        "\"invoice\""
    );
    assert_eq!(
        serde_json::to_string(&MemoType::Payroll).unwrap(),
        "\"payroll\""
    );
    assert_eq!(
        serde_json::to_string(&MemoType::Refund).unwrap(),
        "\"refund\""
    );
    assert_eq!(
        serde_json::to_string(&MemoType::Batch).unwrap(),
        "\"batch\""
    );
    assert_eq!(
        serde_json::to_string(&MemoType::Subscription).unwrap(),
        "\"subscription\""
    );
    assert_eq!(
        serde_json::to_string(&MemoType::Custom).unwrap(),
        "\"custom\""
    );
}

// ── MatchStatus ───────────────────────────────────────────────────────────

#[test]
fn match_status_all_variants_roundtrip() {
    let variants = [
        MatchStatus::Matched,
        MatchStatus::Partial,
        MatchStatus::UnknownMemo,
        MatchStatus::NoMemo,
        MatchStatus::MismatchAmount,
        MatchStatus::MismatchToken,
        MatchStatus::MismatchParty,
        MatchStatus::Expired,
    ];
    for v in &variants {
        assert_eq!(&roundtrip(v), v, "roundtrip failed for {:?}", v);
    }
}

#[test]
fn match_status_uses_snake_case_serialization() {
    assert_eq!(
        serde_json::to_string(&MatchStatus::Matched).unwrap(),
        "\"matched\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::UnknownMemo).unwrap(),
        "\"unknown_memo\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::NoMemo).unwrap(),
        "\"no_memo\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::MismatchAmount).unwrap(),
        "\"mismatch_amount\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::MismatchToken).unwrap(),
        "\"mismatch_token\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::MismatchParty).unwrap(),
        "\"mismatch_party\""
    );
    assert_eq!(
        serde_json::to_string(&MatchStatus::Expired).unwrap(),
        "\"expired\""
    );
}

// ── PaymentEvent ──────────────────────────────────────────────────────────

fn make_payment_event() -> PaymentEvent {
    PaymentEvent {
        chain_id: 42431,
        block_number: 1234,
        tx_hash: "0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string(),
        log_index: 7,
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        from: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        to: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        amount: 10_000_000,
        memo_raw: Some(
            "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string(),
        ),
        memo: None,
        timestamp: Some(1_700_000_000),
    }
}

#[test]
fn payment_event_full_roundtrip() {
    let original = make_payment_event();
    assert_eq!(roundtrip(&original), original);
}

#[test]
fn payment_event_optional_fields_none_roundtrip() {
    let mut ev = make_payment_event();
    ev.memo_raw = None;
    ev.memo = None;
    ev.timestamp = None;
    assert_eq!(roundtrip(&ev), ev);
}

#[test]
fn payment_event_with_decoded_memo_text_roundtrip() {
    let mut ev = make_payment_event();
    ev.memo = Some(Memo::Text("hello-payment".to_string()));
    assert_eq!(roundtrip(&ev), ev);
}

#[test]
fn payment_event_with_memo_v1_roundtrip() {
    let mut ev = make_payment_event();
    ev.memo = Some(Memo::V1(MemoV1 {
        v: 1,
        t: MemoType::Invoice,
        issuer_tag: 0xdeadbeef_cafebabe,
        ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
        id16: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        salt: [0u8; 7],
        raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string(),
    }));
    assert_eq!(roundtrip(&ev), ev);
}

// ── ExpectedPayment ───────────────────────────────────────────────────────

#[test]
fn expected_payment_full_roundtrip() {
    let mut meta = HashMap::new();
    meta.insert("invoice_id".to_string(), "INV-001".to_string());
    meta.insert("customer".to_string(), "acme-corp".to_string());

    let original = ExpectedPayment {
        memo_raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string(),
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        to: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        amount: 99_999_999,
        from: Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        due_at: Some(1_800_000_000),
        meta: Some(meta),
    };
    assert_eq!(roundtrip(&original), original);
}

#[test]
fn expected_payment_optional_fields_none_roundtrip() {
    let original = ExpectedPayment {
        memo_raw: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        to: "0xrecipient".to_string(),
        amount: 0,
        from: None,
        due_at: None,
        meta: None,
    };
    assert_eq!(roundtrip(&original), original);
}

// ── MatchResult ───────────────────────────────────────────────────────────
// MatchResult does not implement PartialEq, so we compare via serde_json::Value.

#[test]
fn match_result_all_optional_fields_set_roundtrip() {
    let original = MatchResult {
        status: MatchStatus::Matched,
        payment: make_payment_event(),
        expected: Some(ExpectedPayment {
            memo_raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
                .to_string(),
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            to: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            amount: 10_000_000,
            from: None,
            due_at: None,
            meta: None,
        }),
        reason: Some("matched within tolerance".to_string()),
        overpaid_by: Some(500_000),
        remaining_amount: None,
        is_late: Some(false),
    };
    roundtrip_via_value(&original);
}

#[test]
fn match_result_optional_fields_none_roundtrip() {
    let original = MatchResult {
        status: MatchStatus::UnknownMemo,
        payment: make_payment_event(),
        expected: None,
        reason: None,
        overpaid_by: None,
        remaining_amount: None,
        is_late: None,
    };
    roundtrip_via_value(&original);
}

#[test]
fn match_result_partial_status_roundtrip() {
    let original = MatchResult {
        status: MatchStatus::Partial,
        payment: make_payment_event(),
        expected: None,
        reason: Some("partial payment accepted".to_string()),
        overpaid_by: None,
        remaining_amount: Some(5_000_000),
        is_late: None,
    };
    roundtrip_via_value(&original);
}

// ── ReconcileSummary ──────────────────────────────────────────────────────
// ReconcileSummary does not implement PartialEq, so we compare via serde_json::Value.

#[test]
fn reconcile_summary_default_roundtrip() {
    roundtrip_via_value(&ReconcileSummary::default());
}

#[test]
fn reconcile_summary_all_fields_set_roundtrip() {
    let original = ReconcileSummary {
        total_expected: 10,
        total_received: 8,
        matched_count: 6,
        issue_count: 2,
        pending_count: 4,
        total_expected_amount: 100_000_000,
        total_received_amount: 80_000_000,
        total_matched_amount: 60_000_000,
        unknown_memo_count: 1,
        no_memo_count: 0,
        mismatch_amount_count: 1,
        mismatch_token_count: 0,
        mismatch_party_count: 0,
        expired_count: 0,
        partial_count: 2,
    };
    roundtrip_via_value(&original);
}

#[test]
fn reconcile_summary_defaults_serialize_as_zeros() {
    let summary = ReconcileSummary::default();
    let json: serde_json::Value = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["total_expected"], 0);
    assert_eq!(json["total_received"], 0);
    assert_eq!(json["matched_count"], 0);
    assert_eq!(json["pending_count"], 0);
    assert_eq!(json["total_expected_amount"], 0);
}

// ── ReconcileReport ───────────────────────────────────────────────────────
// ReconcileReport contains MatchResult which does not implement PartialEq,
// so we compare via serde_json::Value.

#[test]
fn reconcile_report_roundtrip() {
    let matched_payment = make_payment_event();
    let issue_payment = {
        let mut ev = make_payment_event();
        ev.log_index = 1;
        ev.amount = 1_000_000;
        ev
    };
    let pending_expected = ExpectedPayment {
        memo_raw: "0xaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd".to_string(),
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        to: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        amount: 5_000_000,
        from: None,
        due_at: None,
        meta: None,
    };

    let report = ReconcileReport {
        matched: vec![MatchResult {
            status: MatchStatus::Matched,
            payment: matched_payment,
            expected: Some(ExpectedPayment {
                memo_raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
                    .to_string(),
                token: "0x20c0000000000000000000000000000000000000".to_string(),
                to: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
                amount: 10_000_000,
                from: None,
                due_at: None,
                meta: None,
            }),
            reason: None,
            overpaid_by: None,
            remaining_amount: None,
            is_late: Some(false),
        }],
        issues: vec![MatchResult {
            status: MatchStatus::MismatchAmount,
            payment: issue_payment,
            expected: None,
            reason: Some("underpaid".to_string()),
            overpaid_by: None,
            remaining_amount: Some(9_000_000),
            is_late: None,
        }],
        pending: vec![pending_expected],
        summary: ReconcileSummary {
            total_expected: 3,
            total_received: 2,
            matched_count: 1,
            issue_count: 1,
            pending_count: 1,
            total_expected_amount: 25_000_000,
            total_received_amount: 11_000_000,
            total_matched_amount: 10_000_000,
            unknown_memo_count: 0,
            no_memo_count: 0,
            mismatch_amount_count: 1,
            mismatch_token_count: 0,
            mismatch_party_count: 0,
            expired_count: 0,
            partial_count: 0,
        },
    };

    roundtrip_via_value(&report);
}

// ── MemoV1 ────────────────────────────────────────────────────────────────

#[test]
fn memo_v1_full_roundtrip() {
    let original = MemoV1 {
        v: 1,
        t: MemoType::Invoice,
        issuer_tag: 0xdeadbeef_cafebabe,
        ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
        id16: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        salt: [0u8; 7],
        raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string(),
    };
    assert_eq!(roundtrip(&original), original);
}

#[test]
fn memo_v1_non_zero_salt_roundtrip() {
    let original = MemoV1 {
        v: 1,
        t: MemoType::Custom,
        issuer_tag: 0x0102030405060708,
        ulid: "01MASW9NF6YW40J40H289H8580".to_string(),
        id16: [0u8; 16],
        salt: [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00],
        raw: "0x0f0102030405060800000000000000000000000000000000aabbccddeeff00".to_string(),
    };
    assert_eq!(roundtrip(&original), original);
}

// ── Memo enum ─────────────────────────────────────────────────────────────

#[test]
fn memo_v1_variant_roundtrip() {
    let inner = MemoV1 {
        v: 1,
        t: MemoType::Invoice,
        issuer_tag: 0xdeadbeef_cafebabe,
        ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
        id16: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        salt: [0u8; 7],
        raw: "0x0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string(),
    };
    let original = Memo::V1(inner);
    assert_eq!(roundtrip(&original), original);
}

#[test]
fn memo_text_variant_roundtrip() {
    let original = Memo::Text("hello-payment-reference".to_string());
    assert_eq!(roundtrip(&original), original);
}

#[test]
fn memo_text_empty_string_roundtrip() {
    let original = Memo::Text(String::new());
    assert_eq!(roundtrip(&original), original);
}
