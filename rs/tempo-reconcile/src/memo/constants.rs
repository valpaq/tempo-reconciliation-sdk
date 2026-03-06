use crate::types::MemoType;

pub const MEMO_BYTES: usize = 32;
/// Bytes 1..9 (8 bytes).
pub const ISSUER_TAG_OFFSET: usize = 1;
pub const ISSUER_TAG_SIZE: usize = 8;
/// Bytes 9..25 (16 bytes).
pub const ID16_OFFSET: usize = 9;
pub const ID16_SIZE: usize = 16;
/// Bytes 25..32 (7 bytes, default zeros).
pub const SALT_OFFSET: usize = 25;
pub const SALT_SIZE: usize = 7;

/// Denominator for basis-points calculations (1 bps = 0.01%).
pub const BASIS_POINTS: u128 = 10_000;

/// Map a type byte to its MemoType variant. Returns None for reserved/unknown codes.
pub fn type_code_to_memo_type(code: u8) -> Option<MemoType> {
    match code {
        0x01 => Some(MemoType::Invoice),
        0x02 => Some(MemoType::Payroll),
        0x03 => Some(MemoType::Refund),
        0x04 => Some(MemoType::Batch),
        0x05 => Some(MemoType::Subscription),
        0x0F => Some(MemoType::Custom),
        _ => None,
    }
}

/// Map a MemoType to its type byte.
pub fn memo_type_to_code(t: &MemoType) -> u8 {
    match t {
        MemoType::Invoice => 0x01,
        MemoType::Payroll => 0x02,
        MemoType::Refund => 0x03,
        MemoType::Batch => 0x04,
        MemoType::Subscription => 0x05,
        MemoType::Custom => 0x0F,
    }
}
