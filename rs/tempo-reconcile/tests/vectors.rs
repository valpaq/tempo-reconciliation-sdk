/// Golden tests against spec/vectors.json — TEMPO-RECONCILE-MEMO-001 compliance.
///
/// All positive vectors must roundtrip: encode → expected_memo_raw AND decode → all fields match.
/// All negative vectors must return None from decode_memo_v1.
use tempo_reconcile::{
    decode_memo_v1, encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params, MemoType,
};

const VECTORS_JSON: &str = include_str!("../../../spec/vectors.json");

#[derive(serde::Deserialize)]
struct VectorsFile {
    positive: Vec<PositiveVector>,
    negative: Vec<NegativeVector>,
}

#[derive(serde::Deserialize)]
struct PositiveVector {
    name: String,
    #[serde(rename = "type")]
    memo_type: String,
    namespace: String,
    #[serde(rename = "issuerTag")]
    issuer_tag: String, // u64 as decimal string
    #[serde(rename = "issuerTagHex")]
    _issuer_tag_hex: String,
    ulid: String,
    #[serde(rename = "saltHex")]
    salt_hex: String,
    #[serde(rename = "memoRaw")]
    memo_raw: String,
}

#[derive(serde::Deserialize)]
struct NegativeVector {
    name: String,
    #[serde(rename = "memoRaw")]
    memo_raw: String,
    reason: String,
}

fn parse_memo_type(s: &str) -> MemoType {
    match s {
        "invoice" => MemoType::Invoice,
        "payroll" => MemoType::Payroll,
        "refund" => MemoType::Refund,
        "batch" => MemoType::Batch,
        "subscription" => MemoType::Subscription,
        "custom" => MemoType::Custom,
        other => panic!("unknown type in vector: {}", other),
    }
}

fn parse_salt(salt_hex: &str) -> [u8; 7] {
    let bytes = hex::decode(salt_hex).expect("valid salt hex");
    assert_eq!(bytes.len(), 7, "salt must be 7 bytes");
    let mut arr = [0u8; 7];
    arr.copy_from_slice(&bytes);
    arr
}

#[test]
fn positive_vectors_issuer_tag() {
    let file: VectorsFile = serde_json::from_str(VECTORS_JSON).expect("parse vectors.json");
    for v in &file.positive {
        let expected_tag: u64 = v.issuer_tag.parse().unwrap();
        let computed_tag = issuer_tag_from_namespace(&v.namespace);
        assert_eq!(
            computed_tag, expected_tag,
            "[{}] issuerTag mismatch: got {}, expected {}",
            v.name, computed_tag, expected_tag
        );
    }
}

#[test]
fn positive_vectors_encode() {
    let file: VectorsFile = serde_json::from_str(VECTORS_JSON).expect("parse vectors.json");
    for v in &file.positive {
        let params = EncodeMemoV1Params {
            memo_type: parse_memo_type(&v.memo_type),
            issuer_tag: issuer_tag_from_namespace(&v.namespace),
            ulid: v.ulid.clone(),
            salt: Some(parse_salt(&v.salt_hex)),
        };
        let encoded =
            encode_memo_v1(&params).unwrap_or_else(|e| panic!("[{}] encode failed: {}", v.name, e));
        assert_eq!(encoded, v.memo_raw, "[{}] encoded memo mismatch", v.name);
    }
}

#[test]
fn positive_vectors_decode() {
    let file: VectorsFile = serde_json::from_str(VECTORS_JSON).expect("parse vectors.json");
    for v in &file.positive {
        let decoded = decode_memo_v1(&v.memo_raw)
            .unwrap_or_else(|| panic!("[{}] decode returned None for a valid memo", v.name));

        let expected_tag: u64 = v.issuer_tag.parse().unwrap();
        assert_eq!(
            decoded.t,
            parse_memo_type(&v.memo_type),
            "[{}] type mismatch",
            v.name
        );
        assert_eq!(
            decoded.issuer_tag, expected_tag,
            "[{}] issuerTag mismatch",
            v.name
        );
        assert_eq!(decoded.ulid, v.ulid, "[{}] ULID mismatch", v.name);

        let expected_salt = parse_salt(&v.salt_hex);
        assert_eq!(decoded.salt, expected_salt, "[{}] salt mismatch", v.name);

        // raw field should match the original memo_raw (lowercased).
        assert_eq!(
            decoded.raw,
            v.memo_raw.to_ascii_lowercase(),
            "[{}] raw mismatch",
            v.name
        );
    }
}

#[test]
fn negative_vectors_return_none() {
    let file: VectorsFile = serde_json::from_str(VECTORS_JSON).expect("parse vectors.json");
    for v in &file.negative {
        let result = decode_memo_v1(&v.memo_raw);
        assert!(
            result.is_none(),
            "[{}] expected None but got Some(_). Reason: {}",
            v.name,
            v.reason
        );
    }
}
