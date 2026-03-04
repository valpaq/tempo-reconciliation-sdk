use tempo_reconcile::{
    bytes16_to_ulid, decode_memo_v1, encode_memo_v1, issuer_tag_from_namespace, random_salt,
    ulid_to_bytes16, EncodeMemoV1Params, MemoError, MemoType,
};

const VALID_ULID: &str = "01MASW9NF6YW40J40H289H858P";

fn params(memo_type: MemoType, ns: &str) -> EncodeMemoV1Params {
    EncodeMemoV1Params {
        memo_type,
        issuer_tag: issuer_tag_from_namespace(ns),
        ulid: VALID_ULID.to_string(),
        salt: None,
    }
}

fn encode(p: EncodeMemoV1Params) -> String {
    encode_memo_v1(&p).expect("encode should succeed")
}

#[test]
fn produces_0x_prefixed_66_char_hex() {
    let memo = encode(params(MemoType::Invoice, "test"));
    assert!(memo.starts_with("0x"));
    assert_eq!(memo.len(), 66);
    assert!(memo[2..].chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn output_is_lowercase_hex() {
    let memo = encode(params(MemoType::Invoice, "test"));
    assert_eq!(memo, memo.to_ascii_lowercase());
}

#[test]
fn type_bytes() {
    let cases = [
        (MemoType::Invoice, "01"),
        (MemoType::Payroll, "02"),
        (MemoType::Refund, "03"),
        (MemoType::Batch, "04"),
        (MemoType::Subscription, "05"),
        (MemoType::Custom, "0f"),
    ];
    for (t, expected) in cases {
        assert_eq!(&encode(params(t, "ns"))[2..4], expected);
    }
}

#[test]
fn issuer_tag_zero_encodes_as_eight_zero_bytes() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: VALID_ULID.to_string(),
        salt: None,
    };
    assert_eq!(&encode(p)[4..20], "0000000000000000");
}

#[test]
fn issuer_tag_max_u64() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: u64::MAX,
        ulid: VALID_ULID.to_string(),
        salt: None,
    };
    assert_eq!(&encode(p)[4..20], "ffffffffffffffff");
}

#[test]
fn issuer_tag_from_namespace_deterministic() {
    assert_eq!(
        issuer_tag_from_namespace("my-company"),
        issuer_tag_from_namespace("my-company")
    );
    assert_ne!(
        issuer_tag_from_namespace("acme"),
        issuer_tag_from_namespace("example")
    );
}

#[test]
fn empty_namespace_does_not_panic() {
    let _ = issuer_tag_from_namespace("");
}

#[test]
fn default_salt_is_seven_zero_bytes() {
    let memo = encode(params(MemoType::Invoice, "ns"));
    assert_eq!(&memo[52..], "00000000000000");
}

#[test]
fn custom_salt_written_verbatim() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: issuer_tag_from_namespace("ns"),
        ulid: VALID_ULID.to_string(),
        salt: Some([0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03]),
    };
    assert_eq!(&encode(p)[52..], "deadbeef010203");
}

#[test]
fn none_salt_equals_zero_salt() {
    let base = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 42,
        ulid: VALID_ULID.to_string(),
        salt: None,
    };
    let explicit = EncodeMemoV1Params {
        salt: Some([0u8; 7]),
        ..EncodeMemoV1Params {
            memo_type: MemoType::Invoice,
            issuer_tag: 42,
            ulid: VALID_ULID.to_string(),
            salt: None,
        }
    };
    assert_eq!(encode(base), encode(explicit));
}

#[test]
fn error_on_ulid_too_short() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "01MASW9NF6YW40J40H289H858".to_string(),
        salt: None,
    };
    assert!(matches!(encode_memo_v1(&p), Err(MemoError::InvalidUlid(_))));
}

#[test]
fn error_on_ulid_too_long() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "01MASW9NF6YW40J40H289H858PX".to_string(),
        salt: None,
    };
    assert!(matches!(encode_memo_v1(&p), Err(MemoError::InvalidUlid(_))));
}

#[test]
fn error_on_invalid_crockford_char() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "01MASW9NF6YW40J40H289H858U".to_string(),
        salt: None,
    };
    assert!(matches!(encode_memo_v1(&p), Err(MemoError::InvalidUlid(_))));
}

#[test]
fn accepts_lowercase_ulid() {
    let p = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "01masw9nf6yw40j40h289h858p".to_string(),
        salt: None,
    };
    assert!(encode_memo_v1(&p).is_ok());
}

#[test]
fn crockford_aliases_produce_same_output() {
    let canonical = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
        salt: None,
    };
    let aliased = EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "O1MASW9NF6YW40J40H289H858P".to_string(), // O → 0
        salt: None,
    };
    assert_eq!(encode(canonical), encode(aliased));
}

#[test]
fn roundtrip_all_types() {
    for memo_type in [
        MemoType::Invoice,
        MemoType::Payroll,
        MemoType::Refund,
        MemoType::Batch,
        MemoType::Subscription,
        MemoType::Custom,
    ] {
        let p = EncodeMemoV1Params {
            memo_type: memo_type.clone(),
            issuer_tag: issuer_tag_from_namespace("rt"),
            ulid: VALID_ULID.to_string(),
            salt: Some([1, 2, 3, 4, 5, 6, 7]),
        };
        let encoded = encode(p);
        let decoded = decode_memo_v1(&encoded).expect("roundtrip decode");
        assert_eq!(decoded.t, memo_type);
        assert_eq!(decoded.ulid, VALID_ULID);
        assert_eq!(decoded.salt, [1, 2, 3, 4, 5, 6, 7]);
    }
}

#[test]
fn random_salt_is_seven_bytes() {
    assert_eq!(random_salt().len(), 7);
}

#[test]
fn two_random_salts_differ() {
    assert_ne!(random_salt(), random_salt());
}

// ── ULID conversions ──────────────────────────────────────────────────────

#[test]
fn ulid_roundtrip() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let id16 = ulid_to_bytes16(ulid).unwrap();
    let back = bytes16_to_ulid(&id16);
    assert_eq!(back, ulid);
}

#[test]
fn ulid_error_on_wrong_length() {
    assert!(ulid_to_bytes16("tooshort").is_err());
    assert!(ulid_to_bytes16("01MASW9NF6YW40J40H289H858PEXTRA").is_err());
}

#[test]
fn ulid_error_on_invalid_crockford_char() {
    // 'U' is not in the Crockford alphabet
    assert!(ulid_to_bytes16("01MASW9NF6YW40J40H289H858U").is_err());
}

#[test]
fn ulid_crockford_aliases_decoded_identically() {
    // O→0, I→1, L→1 aliases produce the same bytes as canonical chars
    let with_aliases = "O1MASW9NF6YW40J40H289H858P";
    let canonical = "01MASW9NF6YW40J40H289H858P";
    assert_eq!(
        ulid_to_bytes16(with_aliases).unwrap(),
        ulid_to_bytes16(canonical).unwrap()
    );
}

#[test]
fn ulid_first_char_7_at_boundary_accepted() {
    // '7' is the maximum valid first char in a standard ULID (time bits constraint).
    // Our encoder stores raw bytes — it accepts any valid Crockford chars.
    let result = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "7ZZZZZZZZZZZZZZZZZZZZZZZZZ".to_string(),
        salt: None,
    });
    assert!(result.is_ok(), "ULID starting with '7' must be accepted");
}

#[test]
fn ulid_first_char_8_above_timestamp_range_accepted() {
    // '8' is valid Crockford base32 but exceeds the ULID timestamp range.
    // Our encoder performs no semantic timestamp validation — stores as bytes only.
    let result = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: "8ZZZZZZZZZZZZZZZZZZZZZZZZZ".to_string(),
        salt: None,
    });
    assert!(
        result.is_ok(),
        "ULID starting with '8' must be accepted (no timestamp check)"
    );
}

#[test]
fn issuer_tag_unicode_namespace_does_not_panic() {
    let tag = issuer_tag_from_namespace("пайроллинг/2024 \u{1F680}");
    let _ = tag; // just ensure it doesn't panic
}

#[test]
fn issuer_tag_empty_string_does_not_panic() {
    let _ = issuer_tag_from_namespace("");
}
