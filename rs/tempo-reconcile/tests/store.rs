use std::collections::HashMap;
use tempo_reconcile::{
    ExpectedPayment, InMemoryStore, MatchResult, MatchStatus, PaymentEvent, ReconcileStore,
};

fn make_expected(memo_raw: &str, amount: u128) -> tempo_reconcile::ExpectedPayment {
    tempo_reconcile::ExpectedPayment {
        memo_raw: memo_raw.to_string(),
        token: "0xtoken".to_string(),
        to: "0xrecipient".to_string(),
        amount,
        from: None,
        due_at: None,
        meta: None,
    }
}

fn make_result(status: MatchStatus) -> MatchResult {
    MatchResult {
        status,
        payment: PaymentEvent {
            chain_id: 42431,
            block_number: 1,
            tx_hash: "0xdeadbeef".to_string(),
            log_index: 0,
            token: "0xtoken".to_string(),
            from: "0xsender".to_string(),
            to: "0xrecipient".to_string(),
            amount: 1_000_000,
            memo_raw: None,
            memo: None,
            timestamp: None,
        },
        expected: None,
        reason: None,
        overpaid_by: None,
        remaining_amount: None,
        is_late: None,
    }
}

// ── expected payments ─────────────────────────────────────────────────────

#[test]
fn add_and_get_expected() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xabc", 1_000)).unwrap();
    assert!(store.get_expected("0xabc").is_some());
    assert_eq!(store.get_expected("0xabc").unwrap().amount, 1_000);
}

#[test]
fn get_expected_returns_none_for_unknown() {
    let store = InMemoryStore::new();
    assert!(store.get_expected("0xunknown").is_none());
}

#[test]
fn add_expected_case_insensitive_key() {
    let mut store = InMemoryStore::new();
    store
        .add_expected(make_expected("0xABCDEF", 1_000))
        .unwrap();
    // lookup is case-insensitive
    assert!(store.get_expected("0xabcdef").is_some());
    assert!(store.get_expected("0xABCDEF").is_some());
}

#[test]
fn duplicate_expected_errors() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xabc", 1_000)).unwrap();
    let err = store.add_expected(make_expected("0xabc", 2_000));
    assert!(err.is_err());
}

#[test]
fn duplicate_expected_case_insensitive() {
    let mut store = InMemoryStore::new();
    store
        .add_expected(make_expected("0xABCDEF", 1_000))
        .unwrap();
    // lowercase version should also be a duplicate
    let err = store.add_expected(make_expected("0xabcdef", 2_000));
    assert!(err.is_err());
}

#[test]
fn remove_expected_returns_true_if_existed() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xabc", 1_000)).unwrap();
    assert!(store.remove_expected("0xabc"));
    assert!(store.get_expected("0xabc").is_none());
}

#[test]
fn remove_expected_returns_false_if_missing() {
    let mut store = InMemoryStore::new();
    assert!(!store.remove_expected("0xnone"));
}

#[test]
fn get_all_expected_returns_all() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xaaa", 1_000)).unwrap();
    store.add_expected(make_expected("0xbbb", 2_000)).unwrap();
    store.add_expected(make_expected("0xccc", 3_000)).unwrap();
    assert_eq!(store.get_all_expected().len(), 3);
}

#[test]
fn get_all_expected_empty_initially() {
    let store = InMemoryStore::new();
    assert!(store.get_all_expected().is_empty());
}

// ── match results ─────────────────────────────────────────────────────────

#[test]
fn add_and_get_result() {
    let mut store = InMemoryStore::new();
    store.add_result("0xhash:0", make_result(MatchStatus::Matched));
    let r = store.get_result("0xhash:0");
    assert!(r.is_some());
    assert_eq!(r.unwrap().status, MatchStatus::Matched);
}

#[test]
fn get_result_returns_none_for_unknown_key() {
    let store = InMemoryStore::new();
    assert!(store.get_result("0xhash:0").is_none());
}

#[test]
fn add_result_overwrites_existing_key() {
    let mut store = InMemoryStore::new();
    store.add_result("key", make_result(MatchStatus::UnknownMemo));
    store.add_result("key", make_result(MatchStatus::Matched));
    assert_eq!(
        store.get_result("key").unwrap().status,
        MatchStatus::Matched
    );
}

#[test]
fn get_all_results_returns_all() {
    let mut store = InMemoryStore::new();
    store.add_result("k1", make_result(MatchStatus::Matched));
    store.add_result("k2", make_result(MatchStatus::NoMemo));
    assert_eq!(store.get_all_results().len(), 2);
}

// ── partial accumulation ──────────────────────────────────────────────────

#[test]
fn add_partial_returns_cumulative() {
    let mut store = InMemoryStore::new();
    assert_eq!(store.add_partial("0xmemo", 100), 100);
    assert_eq!(store.add_partial("0xmemo", 200), 300);
    assert_eq!(store.add_partial("0xmemo", 50), 350);
}

#[test]
fn get_partial_total_returns_zero_for_unknown() {
    let store = InMemoryStore::new();
    assert_eq!(store.get_partial_total("0xunknown"), 0);
}

#[test]
fn remove_partial_clears_accumulation() {
    let mut store = InMemoryStore::new();
    store.add_partial("0xmemo", 500);
    store.remove_partial("0xmemo");
    assert_eq!(store.get_partial_total("0xmemo"), 0);
}

#[test]
fn remove_partial_noop_for_unknown() {
    let mut store = InMemoryStore::new();
    // should not panic
    store.remove_partial("0xunknown");
}

#[test]
fn partials_are_independent_per_memo() {
    let mut store = InMemoryStore::new();
    store.add_partial("0xmemo1", 1_000);
    store.add_partial("0xmemo2", 5_000);
    assert_eq!(store.get_partial_total("0xmemo1"), 1_000);
    assert_eq!(store.get_partial_total("0xmemo2"), 5_000);
}

#[test]
fn partial_case_insensitive_key() {
    let mut store = InMemoryStore::new();
    store.add_partial("0xABCDEF", 100);
    assert_eq!(store.get_partial_total("0xabcdef"), 100);
    store.add_partial("0xabcdef", 50);
    assert_eq!(store.get_partial_total("0xABCDEF"), 150);
}

// ── clear ─────────────────────────────────────────────────────────────────

#[test]
fn clear_removes_all_expected() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xaaa", 1_000)).unwrap();
    store.add_expected(make_expected("0xbbb", 2_000)).unwrap();
    store.clear();
    assert!(store.get_all_expected().is_empty());
}

#[test]
fn clear_removes_all_results() {
    let mut store = InMemoryStore::new();
    store.add_result("k1", make_result(MatchStatus::Matched));
    store.clear();
    assert!(store.get_all_results().is_empty());
}

#[test]
fn clear_removes_all_partials() {
    let mut store = InMemoryStore::new();
    store.add_partial("0xmemo", 9_999);
    store.clear();
    assert_eq!(store.get_partial_total("0xmemo"), 0);
}

#[test]
fn can_reuse_memo_raw_after_clear() {
    let mut store = InMemoryStore::new();
    store.add_expected(make_expected("0xabc", 1_000)).unwrap();
    store.clear();
    // should not error after clear
    store.add_expected(make_expected("0xabc", 2_000)).unwrap();
    assert_eq!(store.get_expected("0xabc").unwrap().amount, 2_000);
}

#[test]
fn add_partial_zero_amount_accumulates_zero() {
    let mut store = InMemoryStore::new();
    assert_eq!(store.add_partial("0xmemo", 0), 0);
    assert_eq!(store.add_partial("0xmemo", 0), 0);
    assert_eq!(store.get_partial_total("0xmemo"), 0);
}

#[test]
fn all_eight_match_statuses_storable() {
    let mut store = InMemoryStore::new();
    let statuses = [
        MatchStatus::Matched,
        MatchStatus::MismatchAmount,
        MatchStatus::MismatchToken,
        MatchStatus::MismatchParty,
        MatchStatus::UnknownMemo,
        MatchStatus::NoMemo,
        MatchStatus::Expired,
        MatchStatus::Partial,
    ];
    for (i, status) in statuses.iter().enumerate() {
        let key = format!("0xtx:{i}");
        let result = make_result(status.clone());
        store.add_result(&key, result);
        assert_eq!(store.get_result(&key).unwrap().status, *status);
    }
    assert_eq!(store.get_all_results().len(), 8);
}

#[test]
fn result_optional_fields_preserved() {
    let mut store = InMemoryStore::new();
    let mut result = make_result(MatchStatus::MismatchAmount);
    result.overpaid_by = Some(500);
    result.remaining_amount = Some(200);
    result.is_late = Some(true);
    result.reason = Some("test reason".to_string());
    store.add_result("0xtx:0", result);
    let stored = store.get_result("0xtx:0").unwrap();
    assert_eq!(stored.overpaid_by, Some(500));
    assert_eq!(stored.remaining_amount, Some(200));
    assert_eq!(stored.is_late, Some(true));
    assert_eq!(stored.reason.as_deref(), Some("test reason"));
}

#[test]
fn expected_optional_fields_preserved() {
    let mut store = InMemoryStore::new();
    let expected = ExpectedPayment {
        memo_raw: "0xabc".to_string(),
        token: "0xtoken".to_string(),
        to: "0xrecipient".to_string(),
        amount: 1000,
        from: Some("0xsender".to_string()),
        due_at: Some(1234567890),
        meta: Some(HashMap::from([("key1".to_string(), "value1".to_string())])),
    };
    store.add_expected(expected).unwrap();
    let stored = store.get_expected("0xabc").unwrap();
    assert_eq!(stored.from.as_deref(), Some("0xsender"));
    assert_eq!(stored.due_at, Some(1234567890));
    assert!(stored.meta.as_ref().unwrap().contains_key("key1"));
}

#[test]
fn case_insensitive_result_keys() {
    let mut store = InMemoryStore::new();
    let result = make_result(MatchStatus::Matched);
    store.add_result("0xABC:0", result);
    assert!(store.get_result("0xabc:0").is_some());
    assert!(store.get_result("0xABC:0").is_some());
}
