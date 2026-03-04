use sha3::{Digest, Keccak256};

/// Derive a deterministic issuer tag from a namespace string.
///
/// Algorithm: `first 8 bytes of keccak256(utf8(namespace))` as u64 big-endian.
///
/// The same namespace always produces the same tag. Tags prevent cross-application
/// memo collisions without requiring a central registry.
///
/// # Example
/// ```
/// use tempo_reconcile::issuer_tag_from_namespace;
/// let tag = issuer_tag_from_namespace("my-app");
/// ```
pub fn issuer_tag_from_namespace(namespace: &str) -> u64 {
    let hash = Keccak256::digest(namespace.as_bytes());
    // keccak256 always produces 32 bytes; extract first 8 with explicit indexing.
    u64::from_be_bytes([
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7],
    ])
}
