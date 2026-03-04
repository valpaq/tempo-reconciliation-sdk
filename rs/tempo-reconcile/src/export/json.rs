use crate::types::MatchResult;

/// Serialize a MatchResult to a JSON object (as a serde_json::Value).
fn result_to_value(r: &MatchResult) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "status": r.status.as_str(),
        "payment": {
            "chainId": r.payment.chain_id,
            "blockNumber": r.payment.block_number,
            "txHash": r.payment.tx_hash,
            "logIndex": r.payment.log_index,
            "token": r.payment.token,
            "from": r.payment.from,
            "to": r.payment.to,
            "amount": r.payment.amount.to_string(),
            "memoRaw": r.payment.memo_raw,
            "timestamp": r.payment.timestamp,
        }
    });

    if let Some(ref e) = r.expected {
        obj["expected"] = serde_json::json!({
            "memoRaw": e.memo_raw,
            "token": e.token,
            "to": e.to,
            "amount": e.amount.to_string(),
            "from": e.from,
            "dueAt": e.due_at,
            "meta": e.meta,
        });
    }

    if let Some(ref reason) = r.reason {
        obj["reason"] = serde_json::Value::String(reason.clone());
    }
    if let Some(v) = r.overpaid_by {
        obj["overpaidBy"] = serde_json::Value::String(v.to_string());
    }
    if let Some(v) = r.remaining_amount {
        obj["remainingAmount"] = serde_json::Value::String(v.to_string());
    }
    if let Some(v) = r.is_late {
        obj["isLate"] = serde_json::Value::Bool(v);
    }

    obj
}

/// Export a slice of MatchResult as a pretty-printed JSON array (2-space indent).
///
/// u128 amounts are serialized as strings to avoid precision loss.
pub fn export_json(results: &[MatchResult]) -> String {
    let arr: Vec<serde_json::Value> = results.iter().map(result_to_value).collect();
    // serde_json::to_string_pretty on a serde_json::Value never fails.
    // Value has no non-serializable types (no cycles, no maps with non-string keys).
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Export a slice of MatchResult as JSONL (one JSON object per line, newline-terminated).
pub fn export_jsonl(results: &[MatchResult]) -> String {
    results
        .iter()
        .map(|r| serde_json::to_string(&result_to_value(r)).unwrap_or_else(|_| "{}".to_string()))
        .collect::<Vec<_>>()
        .join("\n")
        + if results.is_empty() { "" } else { "\n" }
}
