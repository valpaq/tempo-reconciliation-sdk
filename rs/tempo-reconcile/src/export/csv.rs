use crate::memo::decode::decode_memo_v1;
use crate::types::{MatchResult, Memo};
use std::collections::BTreeSet;

/// CSV escape: wrap in double-quotes if the value contains comma, quote, or newline.
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Format a raw u128 token amount as a decimal string with 6 decimal places.
///
/// TIP-20 tokens always have 6 decimals. 10_000_000 → "10.000000".
fn format_amount_human(raw: u128) -> String {
    let whole = raw / 1_000_000;
    let frac = raw % 1_000_000;
    format!("{}.{:06}", whole, frac)
}

/// Extract decoded memo columns (type, ulid, issuer_tag) from a PaymentEvent.
fn memo_columns(event: &crate::types::PaymentEvent) -> (String, String, String) {
    match &event.memo {
        Some(Memo::V1(m)) => (
            m.t.as_str().to_string(),
            m.ulid.clone(),
            m.issuer_tag.to_string(),
        ),
        Some(Memo::Text(_)) => ("text".to_string(), String::new(), String::new()),
        None => match &event.memo_raw {
            Some(raw) => match decode_memo_v1(raw) {
                Some(m) => (m.t.as_str().to_string(), m.ulid, m.issuer_tag.to_string()),
                None => Default::default(),
            },
            None => Default::default(),
        },
    }
}

/// Extract expected payment columns (amount, from, to, due_at) from an optional ExpectedPayment.
fn expected_columns(
    expected: &Option<crate::types::ExpectedPayment>,
) -> (String, String, String, String) {
    match expected {
        Some(e) => (
            e.amount.to_string(),
            e.from.clone().unwrap_or_default(),
            e.to.clone(),
            e.due_at.map(|d| d.to_string()).unwrap_or_default(),
        ),
        None => Default::default(),
    }
}

/// Export a slice of MatchResult as a CSV string.
///
/// Fixed columns + dynamic `meta_*` columns (one per unique key across all results).
pub fn export_csv(results: &[MatchResult]) -> String {
    let meta_keys: BTreeSet<String> = results
        .iter()
        .filter_map(|r| r.expected.as_ref())
        .filter_map(|e| e.meta.as_ref())
        .flat_map(|m| m.keys().cloned())
        .collect();

    let meta_keys: Vec<String> = meta_keys.into_iter().collect();

    let base_headers = vec![
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
    ];

    let mut header_row: Vec<String> = base_headers.iter().map(|s| s.to_string()).collect();
    for k in &meta_keys {
        header_row.push(format!("meta_{}", k));
    }

    let mut rows: Vec<String> = vec![header_row.join(",")];

    for r in results {
        let p = &r.payment;
        let (memo_type, memo_ulid, memo_issuer_tag) = memo_columns(p);
        let (exp_amount, exp_from, exp_to, exp_due_at) = expected_columns(&r.expected);
        let memo_raw = p.memo_raw.as_deref().unwrap_or("");

        let mut cols = vec![
            csv_escape(&p.timestamp.map(|t| t.to_string()).unwrap_or_default()),
            csv_escape(&p.block_number.to_string()),
            csv_escape(&p.tx_hash),
            csv_escape(&p.log_index.to_string()),
            csv_escape(&p.chain_id.to_string()),
            csv_escape(&p.from),
            csv_escape(&p.to),
            csv_escape(&p.token),
            csv_escape(&p.amount.to_string()),
            csv_escape(&format_amount_human(p.amount)),
            csv_escape(memo_raw),
            csv_escape(&memo_type),
            csv_escape(&memo_ulid),
            csv_escape(&memo_issuer_tag),
            csv_escape(r.status.as_str()),
            csv_escape(&exp_amount),
            csv_escape(&exp_from),
            csv_escape(&exp_to),
            csv_escape(&exp_due_at),
            csv_escape(r.reason.as_deref().unwrap_or("")),
            csv_escape(&r.overpaid_by.map(|v| v.to_string()).unwrap_or_default()),
            csv_escape(&r.is_late.map(|v| v.to_string()).unwrap_or_default()),
            csv_escape(
                &r.remaining_amount
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
            ),
        ];

        for k in &meta_keys {
            let val = r
                .expected
                .as_ref()
                .and_then(|e| e.meta.as_ref())
                .and_then(|m| m.get(k))
                .map(|s| s.as_str())
                .unwrap_or("");
            cols.push(csv_escape(val));
        }

        rows.push(cols.join(","));
    }

    rows.join("\n") + "\n"
}
