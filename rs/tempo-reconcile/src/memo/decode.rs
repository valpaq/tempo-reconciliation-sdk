use super::constants::{
    type_code_to_memo_type, ID16_OFFSET, ID16_SIZE, ISSUER_TAG_OFFSET, SALT_OFFSET,
};
use super::ulid::bytes16_to_ulid;
use crate::types::{Memo, MemoV1};

/// Decode a bytes32 hex string as a v1 structured memo.
///
/// Returns `None` (never panics) for any input that is not a valid v1 memo:
/// wrong length, invalid hex, reserved type code, unknown type code, invalid ULID bytes.
///
/// # Example
/// ```
/// use tempo_reconcile::decode_memo_v1;
/// let raw = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
/// let memo = decode_memo_v1(raw).unwrap();
/// assert_eq!(memo.ulid, "01MASW9NF6YW40J40H289H858P");
/// ```
pub fn decode_memo_v1(memo_raw: &str) -> Option<MemoV1> {
    // Must be "0x" + 64 hex chars = 66 chars total (32 bytes).
    if !memo_raw.starts_with("0x") || memo_raw.len() != 66 {
        return None;
    }

    let bytes = hex::decode(&memo_raw[2..]).ok()?;
    if bytes.len() != 32 {
        return None;
    }

    let t = type_code_to_memo_type(bytes[0])?;

    let issuer_tag = u64::from_be_bytes(
        bytes[ISSUER_TAG_OFFSET..ISSUER_TAG_OFFSET + 8]
            .try_into()
            .ok()?,
    );

    let id16: [u8; ID16_SIZE] = bytes[ID16_OFFSET..ID16_OFFSET + ID16_SIZE]
        .try_into()
        .ok()?;

    // bytes16_to_ulid never errors; it always produces a 26-char string.
    let ulid = bytes16_to_ulid(&id16);

    let salt: [u8; 7] = bytes[SALT_OFFSET..SALT_OFFSET + 7].try_into().ok()?;

    Some(MemoV1 {
        v: 1,
        t,
        issuer_tag,
        ulid,
        id16,
        salt,
        raw: memo_raw.to_ascii_lowercase(),
    })
}

/// Decode a bytes32 hex string as a UTF-8 text memo.
///
/// Handles both left-zero-padded and right-zero-padded strings.
/// Returns `None` if the bytes are not valid printable UTF-8 after stripping padding.
pub fn decode_memo_text(memo_raw: &str) -> Option<String> {
    if !memo_raw.starts_with("0x") || memo_raw.len() != 66 {
        return None;
    }

    let bytes = hex::decode(&memo_raw[2..]).ok()?;

    // Try stripping trailing zero bytes first (right-padded).
    let trimmed_right: Vec<u8> = bytes
        .iter()
        .copied()
        .rev()
        .skip_while(|&b| b == 0)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if !trimmed_right.is_empty() {
        if let Ok(s) = std::str::from_utf8(&trimmed_right) {
            if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                return Some(s.to_string());
            }
        }
    }

    // Try stripping leading zero bytes (left-padded).
    let trimmed_left: Vec<u8> = bytes.iter().copied().skip_while(|&b| b == 0).collect();
    if !trimmed_left.is_empty() {
        if let Ok(s) = std::str::from_utf8(&trimmed_left) {
            if s.chars().all(|c| !c.is_control() || c == '\n' || c == '\t') {
                return Some(s.to_string());
            }
        }
    }

    None
}

/// Decode a bytes32 hex string: try v1 first, then UTF-8 text.
///
/// Returns `None` if neither decoding succeeds.
pub fn decode_memo(memo_raw: &str) -> Option<Memo> {
    if let Some(v1) = decode_memo_v1(memo_raw) {
        return Some(Memo::V1(v1));
    }
    decode_memo_text(memo_raw).map(Memo::Text)
}

/// Return true if memo_raw decodes as a valid v1 structured memo.
pub fn is_memo_v1(memo_raw: &str) -> bool {
    decode_memo_v1(memo_raw).is_some()
}
