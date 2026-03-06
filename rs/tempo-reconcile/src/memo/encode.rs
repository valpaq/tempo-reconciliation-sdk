use super::constants::{
    memo_type_to_code, ID16_OFFSET, ISSUER_TAG_OFFSET, ISSUER_TAG_SIZE, MEMO_BYTES, SALT_OFFSET,
    SALT_SIZE,
};
use super::ulid::ulid_to_bytes16;
use crate::types::MemoType;
use crate::MemoError;

/// Parameters for encoding a v1 memo.
#[derive(Debug)]
pub struct EncodeMemoV1Params {
    pub memo_type: MemoType,
    /// Issuer namespace tag (u64). Derive with [`crate::issuer_tag_from_namespace`].
    pub issuer_tag: u64,
    /// 26-character Crockford base32 ULID string.
    pub ulid: String,
    /// Optional 7-byte salt. `None` encodes as seven zero bytes.
    pub salt: Option<[u8; 7]>,
}

/// Encode memo fields into a 32-byte hex string per TEMPO-RECONCILE-MEMO-001.
///
/// Layout: `[type:1][issuerTag:8][id16:16][salt:7]` = 32 bytes.
///
/// Returns `"0x"` + 64 lowercase hex characters.
///
/// # Errors
/// Returns [`MemoError`] if the ULID is invalid (wrong length, bad chars).
///
/// # Example
/// ```
/// use tempo_reconcile::{encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params};
/// use tempo_reconcile::MemoType;
///
/// let tag = issuer_tag_from_namespace("my-app");
/// let memo = encode_memo_v1(&EncodeMemoV1Params {
///     memo_type: MemoType::Invoice,
///     issuer_tag: tag,
///     ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
///     salt: None,
/// }).unwrap();
/// assert!(memo.starts_with("0x"));
/// assert_eq!(memo.len(), 66);
/// ```
pub fn encode_memo_v1(params: &EncodeMemoV1Params) -> Result<String, MemoError> {
    let id16 = ulid_to_bytes16(&params.ulid)?;

    let salt = params.salt.unwrap_or([0u8; SALT_SIZE]);

    let mut buf = [0u8; MEMO_BYTES];

    buf[0] = memo_type_to_code(&params.memo_type);

    buf[ISSUER_TAG_OFFSET..ISSUER_TAG_OFFSET + ISSUER_TAG_SIZE]
        .copy_from_slice(&params.issuer_tag.to_be_bytes());

    buf[ID16_OFFSET..ID16_OFFSET + 16].copy_from_slice(&id16);
    buf[SALT_OFFSET..SALT_OFFSET + SALT_SIZE].copy_from_slice(&salt);

    Ok(format!("0x{}", hex::encode(buf)))
}

/// Generate 7 cryptographically random bytes for use as a memo salt.
#[cfg(feature = "rand")]
pub fn random_salt() -> [u8; 7] {
    use rand::RngCore;
    let mut buf = [0u8; 7];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}
