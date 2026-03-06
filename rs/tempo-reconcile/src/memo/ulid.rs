/// Crockford base32 alphabet (32 characters).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Decode a Crockford base32 character to its 5-bit value.
/// Handles case-insensitivity and aliases: O→0, I→1, L→1.
fn crockford_decode(ch: char) -> Option<u8> {
    match ch.to_ascii_uppercase() {
        '0' | 'O' => Some(0),
        '1' | 'I' | 'L' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'A' => Some(10),
        'B' => Some(11),
        'C' => Some(12),
        'D' => Some(13),
        'E' => Some(14),
        'F' => Some(15),
        'G' => Some(16),
        'H' => Some(17),
        'J' => Some(18),
        'K' => Some(19),
        'M' => Some(20),
        'N' => Some(21),
        'P' => Some(22),
        'Q' => Some(23),
        'R' => Some(24),
        'S' => Some(25),
        'T' => Some(26),
        'V' => Some(27),
        'W' => Some(28),
        'X' => Some(29),
        'Y' => Some(30),
        'Z' => Some(31),
        _ => None,
    }
}

/// Convert a 26-character Crockford base32 ULID string to 16 bytes.
///
/// The 26 characters encode 130 bits (26 × 5). The top 2 bits are always 0
/// for a valid ULID (128 bits), stored big-endian in the 16-byte result.
///
/// Returns an error if the string is not exactly 26 chars or contains
/// invalid Crockford characters.
pub fn ulid_to_bytes16(ulid: &str) -> Result<[u8; 16], crate::MemoError> {
    let chars: Vec<char> = ulid.chars().collect();
    if chars.len() != 26 {
        return Err(crate::MemoError::InvalidUlid(format!(
            "ULID must be 26 characters, got {}",
            chars.len()
        )));
    }

    // Convert chars to 5-bit indices.
    let mut indices = [0u8; 26];
    for (i, &ch) in chars.iter().enumerate() {
        indices[i] = crockford_decode(ch).ok_or_else(|| {
            crate::MemoError::InvalidUlid(format!(
                "Invalid Crockford character '{}' at position {}",
                ch, i
            ))
        })?;
    }

    // First char must be 0-7 to fit in 128 bits
    if indices[0] > 7 {
        return Err(crate::MemoError::InvalidUlid(
            "first character must be 0-7 (128-bit overflow)".into(),
        ));
    }

    // Accumulate 26 x 5 = 130 bits into a u128 (the top 2 bits are guaranteed
    // zero by the indices[0] <= 7 check above).
    let mut bits: u128 = 0;
    for &idx in &indices {
        bits = (bits << 5) | idx as u128;
    }

    let mut id16 = [0u8; 16];
    for (i, byte) in id16.iter_mut().enumerate() {
        *byte = (bits >> (120 - i * 8)) as u8;
    }
    Ok(id16)
}

/// Convert 16 bytes (big-endian u128) to a 26-character Crockford base32 ULID string.
///
/// Returns an error if the slice is not exactly 16 bytes.
pub fn bytes16_to_ulid(id16: &[u8; 16]) -> String {
    // Convert to u128 big-endian.
    let mut bits: u128 = 0;
    for &b in id16.iter() {
        bits = (bits << 8) | b as u128;
    }

    // Extract 26 Crockford characters, lowest 5 bits first (right-to-left).
    let mut chars = [0u8; 26];
    for ch in chars.iter_mut().rev() {
        *ch = ALPHABET[(bits & 0x1F) as usize];
        bits >>= 5;
    }

    String::from_utf8(chars.to_vec()).expect("ALPHABET is ASCII")
}
