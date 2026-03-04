use proptest::prelude::*;
use tempo_reconcile::{
    decode_memo_v1, encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params, MemoType,
};

fn arb_memo_type() -> impl Strategy<Value = MemoType> {
    prop_oneof![
        Just(MemoType::Invoice),
        Just(MemoType::Payroll),
        Just(MemoType::Refund),
        Just(MemoType::Batch),
        Just(MemoType::Subscription),
        Just(MemoType::Custom),
    ]
}

// Valid 26-char Crockford base32 ULID (first char 0-7, rest 0-9A-HJKMNP-TV-Z).
fn arb_ulid() -> impl Strategy<Value = String> {
    "[0-7][0-9A-HJKMNP-TV-Z]{25}".prop_map(|s| s)
}

proptest! {
    #[test]
    fn memo_roundtrip(
        namespace in "[a-z][a-z0-9\\-]{1,30}",
        ulid in arb_ulid(),
        memo_type in arb_memo_type(),
    ) {
        let issuer_tag = issuer_tag_from_namespace(&namespace);
        let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
            memo_type,
            issuer_tag,
            ulid: ulid.clone(),
            salt: None,
        }).unwrap();
        let decoded = decode_memo_v1(&memo_raw).unwrap();
        prop_assert_eq!(decoded.ulid, ulid);
        prop_assert_eq!(decoded.issuer_tag, issuer_tag);
    }

    #[test]
    fn decode_never_panics(s in ".*") {
        let _ = decode_memo_v1(&s);
    }

    #[test]
    fn encode_always_32_bytes(
        namespace in "[a-z][a-z0-9\\-]{1,30}",
        ulid in arb_ulid(),
        memo_type in arb_memo_type(),
    ) {
        let tag = issuer_tag_from_namespace(&namespace);
        let memo = encode_memo_v1(&EncodeMemoV1Params {
            memo_type,
            issuer_tag: tag,
            ulid,
            salt: None,
        }).unwrap();
        // "0x" prefix + 64 hex chars = 66 chars total = 32 bytes encoded
        prop_assert_eq!(memo.len(), 66);
        prop_assert!(memo.starts_with("0x"));
    }
}
