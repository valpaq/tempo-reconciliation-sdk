use proptest::prelude::*;
use tempo_reconcile::memo::{
    bytes16_to_ulid, decode_memo, decode_memo_text, decode_memo_v1, encode_memo_v1,
    issuer_tag_from_namespace, EncodeMemoV1Params,
};
use tempo_reconcile::types::MemoType;

// Any 32-byte input must never panic when decoded.
proptest! {
    #[test]
    fn decode_v1_never_panics(bytes in prop::array::uniform32(any::<u8>())) {
        let hex = format!("0x{}", hex::encode(bytes));
        let _ = decode_memo_v1(&hex);
    }

    #[test]
    fn decode_text_never_panics(bytes in prop::array::uniform32(any::<u8>())) {
        let hex = format!("0x{}", hex::encode(bytes));
        let _ = decode_memo_text(&hex);
    }

    #[test]
    fn decode_memo_never_panics(bytes in prop::array::uniform32(any::<u8>())) {
        let hex = format!("0x{}", hex::encode(bytes));
        let _ = decode_memo(&hex);
    }

    // Wrong-length inputs must never panic.
    #[test]
    fn decode_v1_wrong_length_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let hex = format!("0x{}", hex::encode(&bytes));
        let _ = decode_memo_v1(&hex);
        let _ = decode_memo_text(&hex);
        let _ = decode_memo(&hex);
    }
}

// Valid encode always produces a decodable memo.
proptest! {
    #[test]
    fn encode_decode_roundtrip(
        type_idx in 0usize..6,
        issuer_ns in "[a-z0-9]{1,32}",
        // Constrain first byte to 0..=7 so the ULID fits in 128 bits (first
        // Crockford char must be 0-7).
        first_byte in 0u8..=7,
        rest in proptest::array::uniform15(any::<u8>()),
        salt in proptest::array::uniform7(any::<u8>()),
    ) {
        let memo_types = [
            MemoType::Invoice,
            MemoType::Payroll,
            MemoType::Refund,
            MemoType::Batch,
            MemoType::Subscription,
            MemoType::Custom,
        ];
        let memo_type = memo_types[type_idx].clone();
        let issuer_tag = issuer_tag_from_namespace(&issuer_ns);

        // Build a valid 16-byte id with first byte constrained.
        let mut id16 = [0u8; 16];
        id16[0] = first_byte;
        id16[1..].copy_from_slice(&rest);

        let ulid = bytes16_to_ulid(&id16);

        let params = EncodeMemoV1Params {
            memo_type: memo_type.clone(),
            issuer_tag,
            ulid: ulid.clone(),
            salt: Some(salt),
        };

        let encoded = encode_memo_v1(&params).unwrap();

        let decoded = decode_memo_v1(&encoded);
        prop_assert!(decoded.is_some(), "failed to decode: {}", encoded);
        let decoded = decoded.unwrap();

        prop_assert_eq!(decoded.t, memo_type);
        prop_assert_eq!(decoded.issuer_tag, issuer_tag);
        prop_assert_eq!(decoded.ulid, ulid);
        prop_assert_eq!(decoded.salt, salt);
        prop_assert_eq!(decoded.id16, id16);
    }
}

// issuer_tag_from_namespace is deterministic.
proptest! {
    #[test]
    fn issuer_tag_deterministic(ns in "[a-zA-Z0-9._]{1,64}") {
        let a = issuer_tag_from_namespace(&ns);
        let b = issuer_tag_from_namespace(&ns);
        prop_assert_eq!(a, b);
    }
}

// bytes16_to_ulid -> ulid_to_bytes16 roundtrip (valid ULID range).
proptest! {
    #[test]
    fn ulid_roundtrip(
        first_byte in 0u8..=7,
        rest in proptest::array::uniform15(any::<u8>()),
    ) {
        let mut id16 = [0u8; 16];
        id16[0] = first_byte;
        id16[1..].copy_from_slice(&rest);

        let ulid_str = bytes16_to_ulid(&id16);
        prop_assert_eq!(ulid_str.len(), 26);

        let back = tempo_reconcile::memo::ulid::ulid_to_bytes16(&ulid_str).unwrap();
        prop_assert_eq!(back, id16);
    }
}
