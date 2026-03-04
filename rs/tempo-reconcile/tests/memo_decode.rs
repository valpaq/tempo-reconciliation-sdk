use tempo_reconcile::{
    decode_memo, decode_memo_text, decode_memo_v1, encode_memo_v1, is_memo_v1,
    issuer_tag_from_namespace, EncodeMemoV1Params, Memo, MemoType,
};

const VALID_ULID: &str = "01MASW9NF6YW40J40H289H858P";
const ALL_ZEROS: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

fn make_memo(memo_type: MemoType) -> String {
    encode_memo_v1(&EncodeMemoV1Params {
        memo_type,
        issuer_tag: issuer_tag_from_namespace("test-ns"),
        ulid: VALID_ULID.to_string(),
        salt: None,
    })
    .unwrap()
}

fn text_memo(text: &str) -> String {
    assert!(text.len() <= 32);
    let mut buf = [0u8; 32];
    buf[..text.len()].copy_from_slice(text.as_bytes());
    format!("0x{}", hex::encode(buf))
}

#[test]
fn decodes_all_types() {
    let cases = [
        (MemoType::Invoice, MemoType::Invoice),
        (MemoType::Payroll, MemoType::Payroll),
        (MemoType::Refund, MemoType::Refund),
        (MemoType::Batch, MemoType::Batch),
        (MemoType::Subscription, MemoType::Subscription),
        (MemoType::Custom, MemoType::Custom),
    ];
    for (input, expected) in cases {
        assert_eq!(decode_memo_v1(&make_memo(input)).unwrap().t, expected);
    }
}

#[test]
fn decoded_issuer_tag_matches() {
    let tag = issuer_tag_from_namespace("my-company");
    let raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: tag,
        ulid: VALID_ULID.to_string(),
        salt: None,
    })
    .unwrap();
    assert_eq!(decode_memo_v1(&raw).unwrap().issuer_tag, tag);
}

#[test]
fn decoded_ulid_matches() {
    assert_eq!(
        decode_memo_v1(&make_memo(MemoType::Invoice)).unwrap().ulid,
        VALID_ULID
    );
}

#[test]
fn decoded_salt_zeros_by_default() {
    assert_eq!(
        decode_memo_v1(&make_memo(MemoType::Invoice)).unwrap().salt,
        [0u8; 7]
    );
}

#[test]
fn decoded_salt_matches_when_set() {
    let salt = [0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0x01];
    let raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Invoice,
        issuer_tag: 0,
        ulid: VALID_ULID.to_string(),
        salt: Some(salt),
    })
    .unwrap();
    assert_eq!(decode_memo_v1(&raw).unwrap().salt, salt);
}

#[test]
fn decoded_raw_is_lowercase() {
    let raw = make_memo(MemoType::Invoice);
    let upper = raw.to_ascii_uppercase().replacen("0X", "0x", 1);
    if let Some(decoded) = decode_memo_v1(&upper) {
        assert_eq!(decoded.raw, raw);
    }
}

#[test]
fn roundtrip_preserves_all_fields() {
    let tag = issuer_tag_from_namespace("rt");
    let salt = [1u8, 2, 3, 4, 5, 6, 7];
    let raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: MemoType::Batch,
        issuer_tag: tag,
        ulid: VALID_ULID.to_string(),
        salt: Some(salt),
    })
    .unwrap();
    let d = decode_memo_v1(&raw).unwrap();
    assert_eq!(d.t, MemoType::Batch);
    assert_eq!(d.issuer_tag, tag);
    assert_eq!(d.ulid, VALID_ULID);
    assert_eq!(d.salt, salt);
    assert_eq!(d.raw, raw);
}

#[test]
fn returns_none_all_zeros() {
    assert!(decode_memo_v1(ALL_ZEROS).is_none());
}

#[test]
fn returns_none_reserved_type_codes() {
    for code in [0x00u8, 0x06, 0x07, 0x0E] {
        let raw = format!("0x{:02x}{}", code, "00".repeat(31));
        assert!(
            decode_memo_v1(&raw).is_none(),
            "code 0x{:02x} should be None",
            code
        );
    }
}

#[test]
fn returns_none_wrong_length() {
    assert!(decode_memo_v1("0x01fc7c").is_none());
    assert!(decode_memo_v1(&format!("0x{}", "01".repeat(33))).is_none());
}

#[test]
fn returns_none_missing_prefix() {
    assert!(
        decode_memo_v1("01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000")
            .is_none()
    );
}

#[test]
fn returns_none_invalid_hex() {
    assert!(decode_memo_v1(&format!("0x{}", "ZZ".repeat(32))).is_none());
}

#[test]
fn returns_none_empty() {
    assert!(decode_memo_v1("").is_none());
    assert!(decode_memo_v1("0x").is_none());
}

#[test]
fn never_panics_on_arbitrary_type_bytes() {
    for code in 0x00u8..=0xFF {
        let raw = format!("0x{:02x}{}", code, "00".repeat(31));
        let _ = decode_memo_v1(&raw);
    }
}

#[test]
fn text_memo_right_padded() {
    let raw = text_memo("PAY-001");
    assert_eq!(decode_memo_text(&raw).unwrap(), "PAY-001");
}

#[test]
fn text_memo_all_zeros_is_none() {
    assert!(decode_memo_text(ALL_ZEROS).is_none());
}

#[test]
fn text_memo_binary_garbage_is_none() {
    let raw = format!("0x{}", "ff".repeat(32));
    assert!(decode_memo_text(&raw).is_none());
}

#[test]
fn unified_decode_returns_v1_for_valid_memo() {
    assert!(matches!(
        decode_memo(&make_memo(MemoType::Invoice)),
        Some(Memo::V1(_))
    ));
}

#[test]
fn unified_decode_returns_text() {
    let raw = text_memo("INV-9999");
    assert!(matches!(decode_memo(&raw), Some(Memo::Text(_))));
}

#[test]
fn unified_decode_all_zeros_is_none() {
    assert!(decode_memo(ALL_ZEROS).is_none());
}

#[test]
fn is_memo_v1_valid() {
    assert!(is_memo_v1(&make_memo(MemoType::Invoice)));
    assert!(!is_memo_v1(ALL_ZEROS));
}

#[test]
fn decodes_mixed_case_hex() {
    let raw = make_memo(MemoType::Invoice);
    // Convert to mixed case: uppercase every other hex char after "0x"
    let hex_part = &raw[2..];
    let mixed: String = hex_part
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    let mixed_raw = format!("0x{mixed}");
    let decoded = decode_memo_v1(&mixed_raw);
    assert!(
        decoded.is_some(),
        "mixed-case hex should decode successfully"
    );
    let d = decoded.unwrap();
    assert_eq!(d.t, MemoType::Invoice);
    assert_eq!(d.ulid, VALID_ULID);
    // raw field should be normalized to lowercase
    assert_eq!(d.raw, raw);
}
