use serde_json::Value;

use crate::memo::decode::decode_memo;
use crate::types::PaymentEvent;

/// Decode a `TransferWithMemo` log entry from `eth_getLogs` into a [`PaymentEvent`].
///
/// Event ABI: `TransferWithMemo(address indexed from, address indexed to, uint256 amount, bytes32 indexed memo)`
/// - `topics[0]`: event signature hash
/// - `topics[1]`: `from` address (padded to 32 bytes)
/// - `topics[2]`: `to` address (padded to 32 bytes)
/// - `topics[3]`: `memo` (bytes32)
/// - `data`: 32 bytes, ABI-encoded `uint256 amount`
pub(super) fn decode_log(log: &Value, chain_id: u32, token: &str) -> Option<PaymentEvent> {
    let topics = log["topics"].as_array()?;
    if topics.len() < 4 {
        return None;
    }

    let from = address_from_topic(topics[1].as_str()?)?;
    let to = address_from_topic(topics[2].as_str()?)?;
    let memo_raw = topics[3].as_str()?.to_ascii_lowercase();
    if memo_raw.len() != 66 || !memo_raw.starts_with("0x") {
        return None;
    }

    let amount = amount_from_data(log["data"].as_str()?)?;
    let block_number = hex_to_u64(log["blockNumber"].as_str()?)?;
    let tx_hash = log["transactionHash"].as_str()?.to_ascii_lowercase();
    let log_index = hex_to_u32(log["logIndex"].as_str()?)?;

    let memo = decode_memo(&memo_raw);

    Some(PaymentEvent {
        chain_id,
        block_number,
        tx_hash,
        log_index,
        token: token.to_ascii_lowercase(),
        from,
        to,
        amount,
        memo_raw: Some(memo_raw),
        memo,
        timestamp: None,
    })
}

fn address_from_topic(topic: &str) -> Option<String> {
    // topic = "0x" + 64 hex chars (32 bytes); address occupies the last 20 bytes
    let hex = topic.strip_prefix("0x")?;
    if hex.len() != 64 {
        return None;
    }
    Some(format!("0x{}", &hex[24..]))
}

fn amount_from_data(data: &str) -> Option<u128> {
    // data = "0x" + 64 hex chars (uint256, big-endian); we take the low 16 bytes as u128
    let hex = data.strip_prefix("0x")?;
    if hex.len() != 64 {
        return None;
    }
    // Reject amounts that overflow u128 (upper 16 bytes must be zero)
    if hex[..32].chars().any(|c| c != '0') {
        return None;
    }
    u128::from_str_radix(&hex[32..], 16).ok()
}

fn hex_to_u64(s: &str) -> Option<u64> {
    u64::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

fn hex_to_u32(s: &str) -> Option<u32> {
    u32::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

/// Decode a plain `Transfer(from, to, amount)` log into a [`PaymentEvent`].
///
/// Event ABI: `Transfer(address indexed from, address indexed to, uint256 amount)`
/// - `topics[0]`: event signature hash
/// - `topics[1]`: `from` address (padded to 32 bytes)
/// - `topics[2]`: `to` address (padded to 32 bytes)
/// - `data`: 32 bytes, ABI-encoded `uint256 amount`
pub(super) fn decode_transfer_log(log: &Value, chain_id: u32, token: &str) -> Option<PaymentEvent> {
    let topics = log["topics"].as_array()?;
    if topics.len() < 3 {
        return None;
    }

    let from = address_from_topic(topics[1].as_str()?)?;
    let to = address_from_topic(topics[2].as_str()?)?;
    let amount = amount_from_data(log["data"].as_str()?)?;
    let block_number = hex_to_u64(log["blockNumber"].as_str()?)?;
    let tx_hash = log["transactionHash"].as_str()?.to_ascii_lowercase();
    let log_index = hex_to_u32(log["logIndex"].as_str()?)?;

    Some(PaymentEvent {
        chain_id,
        block_number,
        tx_hash,
        log_index,
        token: token.to_ascii_lowercase(),
        from,
        to,
        amount,
        memo_raw: None,
        memo: None,
        timestamp: None,
    })
}
