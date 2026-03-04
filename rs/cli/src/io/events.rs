use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tempo_reconcile::PaymentEvent;

/// Intermediate struct for camelCase JSONL deserialization.
/// Matches the output format of the TS `watch` command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventRecord {
    chain_id: u32,
    block_number: u64,
    tx_hash: String,
    log_index: u32,
    token: String,
    from: String,
    to: String,
    /// Amount serialized as decimal string (safe for u128 / JS bigint).
    amount: String,
    memo_raw: Option<String>,
    timestamp: Option<u64>,
}

impl TryFrom<EventRecord> for PaymentEvent {
    type Error = anyhow::Error;

    fn try_from(r: EventRecord) -> Result<Self> {
        let amount = r
            .amount
            .parse::<u128>()
            .with_context(|| format!("invalid amount {:?}", r.amount))?;
        Ok(PaymentEvent {
            chain_id: r.chain_id,
            block_number: r.block_number,
            tx_hash: r.tx_hash,
            log_index: r.log_index,
            token: r.token,
            from: r.from,
            to: r.to,
            amount,
            memo_raw: r.memo_raw,
            memo: None,
            timestamp: r.timestamp,
        })
    }
}

/// Parse a JSONL file into a list of payment events.
///
/// Blank lines are skipped. Each non-blank line must be valid JSON
/// matching the `EventRecord` shape.
pub fn read_events(path: &Path) -> Result<Vec<PaymentEvent>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read events file {}", path.display()))?;

    let mut events = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: EventRecord = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: invalid JSON", path.display(), line_no + 1))?;
        events.push(PaymentEvent::try_from(record)?);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_tmp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_single_valid_line() {
        let line = r#"{"chainId":42431,"blockNumber":1,"txHash":"0xabc","logIndex":0,"token":"0x20c0","from":"0xfrom","to":"0xto","amount":"10000000","memoRaw":"0x01fc7c","timestamp":1700000000}"#;
        let f = write_tmp(&format!("{}\n", line));
        let events = read_events(f.path()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].amount, 10_000_000);
        assert_eq!(events[0].chain_id, 42431);
    }

    #[test]
    fn skips_blank_lines() {
        let content = "\n\n{\"chainId\":1,\"blockNumber\":1,\"txHash\":\"0x\",\"logIndex\":0,\"token\":\"0x\",\"from\":\"0x\",\"to\":\"0x\",\"amount\":\"1\"}\n\n";
        let f = write_tmp(content);
        let events = read_events(f.path()).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn error_on_invalid_json() {
        let f = write_tmp("not json\n");
        assert!(read_events(f.path()).is_err());
    }

    #[test]
    fn error_on_invalid_amount() {
        let line = r#"{"chainId":1,"blockNumber":1,"txHash":"0x","logIndex":0,"token":"0x","from":"0x","to":"0x","amount":"not_a_number"}"#;
        let f = write_tmp(&format!("{}\n", line));
        assert!(read_events(f.path()).is_err());
    }

    #[test]
    fn memo_raw_none_when_absent() {
        let line = r#"{"chainId":1,"blockNumber":1,"txHash":"0x","logIndex":0,"token":"0x","from":"0x","to":"0x","amount":"0"}"#;
        let f = write_tmp(&format!("{}\n", line));
        let events = read_events(f.path()).unwrap();
        assert!(events[0].memo_raw.is_none());
    }
}
