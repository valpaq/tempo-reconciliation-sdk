use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use tempo_reconcile::ExpectedPayment;

/// Parse a CSV file of expected payments.
///
/// Required columns: `memo_raw`, `token`, `to`, `amount`.
/// Optional columns: `from`, `due_at`.
/// Dynamic `meta.*` columns are collected into `meta: HashMap<String, String>`.
///
/// CSV format (RFC 4180, quoted fields supported):
/// ```csv
/// memo_raw,token,to,amount,from,due_at,meta.invoiceId,meta.customer
/// 0x01...,0x20c0...,0xrecipient,10000000,,,INV-001,Acme Corp
/// ```
/// Resolved column indices for expected-payment CSV headers.
struct HeaderIndices {
    memo_raw: usize,
    token: usize,
    to: usize,
    amount: usize,
    from: Option<usize>,
    due_at: Option<usize>,
    /// (column_index, stripped_key_name) pairs for `meta.*` columns.
    meta: Vec<(usize, String)>,
}

impl HeaderIndices {
    fn parse(headers: &[String]) -> Result<Self> {
        for required in ["memo_raw", "token", "to", "amount"] {
            if !headers.iter().any(|h| h == required) {
                bail!("expected CSV missing required column: {required}");
            }
        }

        let col = |name: &str| headers.iter().position(|h| h == name);

        let meta = headers
            .iter()
            .enumerate()
            .filter_map(|(i, h)| h.strip_prefix("meta.").map(|k| (i, k.to_string())))
            .collect();

        Ok(Self {
            memo_raw: col("memo_raw").unwrap(),
            token: col("token").unwrap(),
            to: col("to").unwrap(),
            amount: col("amount").unwrap(),
            from: col("from"),
            due_at: col("due_at"),
            meta,
        })
    }
}

pub fn read_expected(path: &Path) -> Result<Vec<ExpectedPayment>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("cannot open expected CSV {}", path.display()))?;
    let mut reader = csv::Reader::from_reader(file);

    let headers: Vec<String> = reader
        .headers()
        .with_context(|| "cannot read CSV headers")?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let idx = HeaderIndices::parse(&headers)?;

    let mut payments = Vec::new();

    for (row_no, record) in reader.records().enumerate() {
        let record = record
            .with_context(|| format!("{}:{}: CSV parse error", path.display(), row_no + 2))?;

        let get = |idx: usize| record.get(idx).unwrap_or("").trim().to_string();
        let get_opt = |idx: usize| {
            let v = record.get(idx).unwrap_or("").trim().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        };

        let memo_raw = get(idx.memo_raw);
        let token = get(idx.token);
        let to = get(idx.to);
        let amount_str = get(idx.amount);

        if memo_raw.is_empty() || token.is_empty() || to.is_empty() || amount_str.is_empty() {
            bail!("{}:{}: required field is empty", path.display(), row_no + 2);
        }

        if !is_valid_memo_raw(&memo_raw) {
            bail!(
                "{}:{}: memo_raw must be 0x followed by 64 hex chars, got {:?}",
                path.display(),
                row_no + 2,
                memo_raw
            );
        }

        let amount = amount_str.parse::<u128>().with_context(|| {
            format!(
                "{}:{}: invalid amount {:?}",
                path.display(),
                row_no + 2,
                amount_str
            )
        })?;

        let from = idx.from.and_then(get_opt);
        let due_at = idx
            .due_at
            .and_then(get_opt)
            .map(|s| s.parse::<u64>())
            .transpose()
            .with_context(|| format!("{}:{}: invalid due_at", path.display(), row_no + 2))?;

        let meta: HashMap<String, String> = idx
            .meta
            .iter()
            .filter_map(|(i, key)| {
                let v = record.get(*i).unwrap_or("").trim().to_string();
                if v.is_empty() {
                    None
                } else {
                    Some((key.clone(), v))
                }
            })
            .collect();

        payments.push(ExpectedPayment {
            memo_raw,
            token,
            to,
            amount,
            from,
            due_at,
            meta: if meta.is_empty() { None } else { Some(meta) },
        });
    }

    Ok(payments)
}

fn is_valid_memo_raw(s: &str) -> bool {
    let hex = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => h,
        None => return false,
    };
    hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Spec vector: invoice, namespace "tempo-reconcile".
    const MEMO: &str = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";

    fn write_tmp(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_minimal_row() {
        let csv = format!("memo_raw,token,to,amount\n{MEMO},0x20c0,0xabc,10000000\n");
        let f = write_tmp(&csv);
        let payments = read_expected(f.path()).unwrap();
        assert_eq!(payments.len(), 1);
        assert_eq!(payments[0].memo_raw, MEMO);
        assert_eq!(payments[0].amount, 10_000_000);
        assert!(payments[0].from.is_none());
        assert!(payments[0].meta.is_none());
    }

    #[test]
    fn parses_all_fields() {
        let csv = format!(
            "memo_raw,token,to,amount,from,due_at\n{MEMO},0x20c0,0xrecipient,25000000,0xsender,1709200000\n"
        );
        let f = write_tmp(&csv);
        let payments = read_expected(f.path()).unwrap();
        assert_eq!(payments[0].from.as_deref(), Some("0xsender"));
        assert_eq!(payments[0].due_at, Some(1_709_200_000));
    }

    #[test]
    fn parses_meta_columns() {
        let csv = format!(
            "memo_raw,token,to,amount,meta.invoiceId,meta.customer\n{MEMO},0x20c0,0xabc,1,INV-001,Acme Corp\n"
        );
        let f = write_tmp(&csv);
        let payments = read_expected(f.path()).unwrap();
        let meta = payments[0].meta.as_ref().unwrap();
        assert_eq!(meta.get("invoiceId").map(|s| s.as_str()), Some("INV-001"));
        assert_eq!(meta.get("customer").map(|s| s.as_str()), Some("Acme Corp"));
    }

    #[test]
    fn error_on_missing_required_column() {
        let csv = "token,to,amount\n0x20c0,0xabc,1\n"; // no memo_raw
        let f = write_tmp(csv);
        assert!(read_expected(f.path()).is_err());
    }

    #[test]
    fn error_on_invalid_amount() {
        let csv = format!("memo_raw,token,to,amount\n{MEMO},0x20c0,0xabc,not_a_number\n");
        let f = write_tmp(&csv);
        assert!(read_expected(f.path()).is_err());
    }

    #[test]
    fn empty_meta_values_skipped() {
        let csv = format!("memo_raw,token,to,amount,meta.invoiceId\n{MEMO},0x20c0,0xabc,1,\n");
        let f = write_tmp(&csv);
        let payments = read_expected(f.path()).unwrap();
        assert!(payments[0].meta.is_none());
    }

    #[test]
    fn error_on_invalid_memo_raw_format() {
        // Too short (not 64 hex chars after 0x).
        let csv = "memo_raw,token,to,amount\n0xdeadbeef,0x20c0,0xabc,1\n";
        let f = write_tmp(csv);
        assert!(read_expected(f.path()).is_err());
    }

    #[test]
    fn error_on_memo_raw_missing_0x_prefix() {
        let memo = "01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
        let csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xabc,1\n");
        let f = write_tmp(&csv);
        assert!(read_expected(f.path()).is_err());
    }

    #[test]
    fn accepts_valid_memo_raw() {
        let memo = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
        let csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xabc,1\n");
        let f = write_tmp(&csv);
        let payments = read_expected(f.path()).unwrap();
        assert_eq!(payments[0].memo_raw, memo);
    }
}
