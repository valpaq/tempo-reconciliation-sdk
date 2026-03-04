#![cfg(feature = "export")]

use std::collections::HashMap;
use tempo_reconcile::{
    export_csv, export_json, export_jsonl, ExpectedPayment, MatchResult, MatchStatus, PaymentEvent,
};

fn make_result(status: MatchStatus, amount: u128) -> MatchResult {
    MatchResult {
        status,
        payment: PaymentEvent {
            chain_id: 42431,
            block_number: 100,
            tx_hash: "0xabc".to_string(),
            log_index: 0,
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            from: "0xfrom".to_string(),
            to: "0xto".to_string(),
            amount,
            memo_raw: None,
            memo: None,
            timestamp: Some(1_700_000_000),
        },
        expected: None,
        reason: None,
        overpaid_by: None,
        remaining_amount: None,
        is_late: None,
    }
}

fn make_result_with_meta(meta: HashMap<String, String>) -> MatchResult {
    let mut r = make_result(MatchStatus::Matched, 10_000_000);
    r.expected = Some(ExpectedPayment {
        memo_raw: "0xmemo".to_string(),
        token: "0xtoken".to_string(),
        to: "0xto".to_string(),
        amount: 10_000_000,
        from: None,
        due_at: None,
        meta: Some(meta),
    });
    r
}

// export_json

#[test]
fn json_empty_is_empty_array() {
    assert_eq!(export_json(&[]).trim(), "[]");
}

#[test]
fn json_valid_json_with_correct_count() {
    let results = vec![
        make_result(MatchStatus::Matched, 1),
        make_result(MatchStatus::NoMemo, 2),
    ];
    let parsed: serde_json::Value = serde_json::from_str(&export_json(&results)).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[test]
fn json_amount_serialized_as_string() {
    let parsed: serde_json::Value = serde_json::from_str(&export_json(&[make_result(
        MatchStatus::Matched,
        10_000_000,
    )]))
    .unwrap();
    assert_eq!(parsed[0]["payment"]["amount"], "10000000");
    assert!(
        parsed[0]["payment"]["blockNumber"].is_number(),
        "blockNumber must be a number"
    );
}

#[test]
fn json_all_status_variants() {
    for status in [
        MatchStatus::Matched,
        MatchStatus::Partial,
        MatchStatus::UnknownMemo,
        MatchStatus::NoMemo,
        MatchStatus::MismatchAmount,
        MatchStatus::MismatchToken,
        MatchStatus::MismatchParty,
        MatchStatus::Expired,
    ] {
        let label = status.as_str().to_string();
        let parsed: serde_json::Value =
            serde_json::from_str(&export_json(&[make_result(status, 0)])).unwrap();
        assert_eq!(parsed[0]["status"], label);
    }
}

#[test]
fn json_optional_fields_present_when_set() {
    let mut r = make_result(MatchStatus::MismatchAmount, 5_000_000);
    r.reason = Some("underpaid".to_string());
    r.remaining_amount = Some(5_000_000);
    let parsed: serde_json::Value = serde_json::from_str(&export_json(&[r])).unwrap();
    assert_eq!(parsed[0]["reason"], "underpaid");
    assert_eq!(parsed[0]["remainingAmount"], "5000000");
}

#[test]
fn json_is_pretty_printed() {
    assert!(
        export_json(&[make_result(MatchStatus::Matched, 0)])
            .lines()
            .count()
            > 1
    );
}

// export_jsonl

#[test]
fn jsonl_empty_is_empty_string() {
    assert_eq!(export_jsonl(&[]), "");
}

#[test]
fn jsonl_one_valid_json_object_per_line() {
    let results = vec![
        make_result(MatchStatus::Matched, 10_000_000),
        make_result(MatchStatus::NoMemo, 0),
        make_result(MatchStatus::UnknownMemo, 1_000),
    ];
    let jsonl = export_jsonl(&results);
    let lines: Vec<&str> = jsonl.lines().collect();
    assert_eq!(lines.len(), 3);
    for line in &lines {
        let _: serde_json::Value =
            serde_json::from_str(line).expect("each line must be valid JSON");
    }
}

#[test]
fn jsonl_newline_terminated() {
    assert!(export_jsonl(&[make_result(MatchStatus::Matched, 1)]).ends_with('\n'));
}

// export_csv

#[test]
fn csv_empty_has_only_header() {
    let csv = export_csv(&[]);
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("tx_hash"));
}

#[test]
fn csv_header_columns() {
    let header = export_csv(&[]);
    for col in [
        "timestamp",
        "block_number",
        "tx_hash",
        "amount_raw",
        "amount_human",
        "memo_type",
        "memo_ulid",
        "memo_issuer_tag",
        "status",
    ] {
        assert!(header.contains(col), "missing: {col}");
    }
}

#[test]
fn csv_header_contains_all_base_columns() {
    let header = export_csv(&[]);
    for col in [
        "timestamp",
        "block_number",
        "tx_hash",
        "log_index",
        "chain_id",
        "from",
        "to",
        "token",
        "amount_raw",
        "amount_human",
        "memo_raw",
        "memo_type",
        "memo_ulid",
        "memo_issuer_tag",
        "status",
        "expected_amount",
        "expected_from",
        "expected_to",
        "expected_due_at",
        "reason",
        "overpaid_by",
        "is_late",
        "remaining_amount",
    ] {
        assert!(header.contains(col), "missing column: {col}");
    }
}

// Known v1 memo vector from spec:
// type=invoice, issuer_tag=18193562290988123368, ulid=01MASW9NF6YW40J40H289H858P
const V1_MEMO_RAW: &str = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";

#[test]
fn csv_memo_columns_populated_for_v1_memo() {
    let result = MatchResult {
        status: MatchStatus::UnknownMemo,
        payment: PaymentEvent {
            chain_id: 42431,
            block_number: 1,
            tx_hash: "0xdeadbeef".to_string(),
            log_index: 0,
            token: "0x20c0000000000000000000000000000000000000".to_string(),
            from: "0xfrom".to_string(),
            to: "0xto".to_string(),
            amount: 10_000_000,
            memo_raw: Some(V1_MEMO_RAW.to_string()),
            memo: None, // force fallback decode in export_csv
            timestamp: Some(1_700_000_000),
        },
        expected: None,
        reason: None,
        overpaid_by: None,
        remaining_amount: None,
        is_late: None,
    };

    let csv = export_csv(&[result]);
    assert!(csv.contains("invoice"), "expected memo_type=invoice");
    assert!(
        csv.contains("01MASW9NF6YW40J40H289H858P"),
        "expected memo_ulid"
    );
    assert!(
        csv.contains("18193562290988123368"),
        "expected memo_issuer_tag"
    );
}

#[test]
fn csv_row_count() {
    let csv = export_csv(&[
        make_result(MatchStatus::Matched, 1),
        make_result(MatchStatus::NoMemo, 2),
        make_result(MatchStatus::UnknownMemo, 3),
    ]);
    assert_eq!(csv.lines().count(), 4); // header + 3 rows
}

#[test]
fn csv_amount_human() {
    assert!(export_csv(&[make_result(MatchStatus::Matched, 10_000_000)]).contains("10.000000"));
    assert!(export_csv(&[make_result(MatchStatus::Matched, 1_500_000)]).contains("1.500000"));
    assert!(export_csv(&[make_result(MatchStatus::Matched, 1)]).contains("0.000001"));
    assert!(export_csv(&[make_result(MatchStatus::NoMemo, 0)]).contains("0.000000"));
}

#[test]
fn csv_meta_columns_and_values() {
    let mut meta = HashMap::new();
    meta.insert("invoice_id".to_string(), "INV-001".to_string());
    let csv = export_csv(&[make_result_with_meta(meta)]);
    assert!(csv.contains("meta_invoice_id"));
    assert!(csv.contains("INV-001"));
}

#[test]
fn csv_multiple_meta_keys() {
    let mut a = HashMap::new();
    a.insert("inv".to_string(), "INV-001".to_string());
    let mut b = HashMap::new();
    b.insert("ord".to_string(), "ORD-002".to_string());
    let csv = export_csv(&[make_result_with_meta(a), make_result_with_meta(b)]);
    assert!(csv.contains("meta_inv"));
    assert!(csv.contains("meta_ord"));
}

#[test]
fn csv_escaping() {
    let mut r = make_result(MatchStatus::MismatchAmount, 0);
    r.reason = Some("wrong,amount".to_string());
    assert!(export_csv(&[r]).contains("\"wrong,amount\""));

    let mut r2 = make_result(MatchStatus::MismatchAmount, 0);
    r2.reason = Some("say \"hi\"".to_string());
    assert!(export_csv(&[r2]).contains("\"say \"\"hi\"\"\""));
}

#[test]
fn csv_newline_terminated() {
    assert!(export_csv(&[]).ends_with('\n'));
}

#[test]
fn csv_amount_human_does_not_panic_on_max_u128() {
    let csv = export_csv(&[make_result(MatchStatus::Matched, u128::MAX)]);
    assert_eq!(csv.lines().count(), 2); // header + 1 data row
                                        // u128::MAX / 1_000_000 starts with "340282366920938463463"
    assert!(csv.contains("340282366920938463463"));
}

#[test]
fn csv_escapes_newline_in_reason() {
    let mut r = make_result(MatchStatus::MismatchAmount, 0);
    r.reason = Some("line1\nline2".to_string());
    let csv = export_csv(&[r]);
    assert!(
        csv.contains("\"line1\nline2\""),
        "newline in field must be wrapped in quotes"
    );
}

#[test]
fn json_optional_fields_absent_when_none() {
    let r = make_result(MatchStatus::Matched, 10_000_000);
    // make_result produces: reason=None, overpaid_by=None, remaining_amount=None, is_late=None
    let parsed: serde_json::Value = serde_json::from_str(&export_json(&[r])).unwrap();
    assert!(
        parsed[0].get("reason").is_none(),
        "reason must be absent when None"
    );
    assert!(
        parsed[0].get("overpaidBy").is_none(),
        "overpaidBy must be absent when None"
    );
    assert!(
        parsed[0].get("remainingAmount").is_none(),
        "remainingAmount must be absent when None"
    );
    assert!(
        parsed[0].get("isLate").is_none(),
        "isLate must be absent when None"
    );
}

#[test]
fn csv_handles_unicode_in_reason_and_meta() {
    let mut r = make_result(MatchStatus::MismatchAmount, 5_000_000);
    r.reason = Some("Payment \u{2713} done \u{2014} \u{a5}500".to_string());
    let mut meta = HashMap::new();
    meta.insert("note".to_string(), "\u{c9}mile's caf\u{e9}".to_string());
    r.expected = Some(ExpectedPayment {
        memo_raw: "0xmemo".to_string(),
        token: "0xtoken".to_string(),
        to: "0xto".to_string(),
        amount: 10_000_000,
        from: None,
        due_at: None,
        meta: Some(meta),
    });
    let csv = export_csv(&[r]);
    assert!(
        csv.contains("Payment \u{2713} done"),
        "Unicode in reason must survive CSV export"
    );
    assert!(
        csv.contains("\u{c9}mile's caf\u{e9}"),
        "Unicode in meta must survive CSV export"
    );
}
