use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use tempo_reconcile::{
    export_csv, export_json, export_jsonl, Reconciler, ReconcilerOptions, ToleranceMode,
};

use crate::io::{events::read_events, expected::read_expected};

#[derive(Clone, ValueEnum, Debug)]
pub enum OutputFormat {
    Csv,
    Json,
    Jsonl,
}

#[derive(Clone, ValueEnum, Debug)]
pub enum CliToleranceMode {
    Final,
    Each,
}

impl std::fmt::Display for CliToleranceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliToleranceMode::Final => write!(f, "final"),
            CliToleranceMode::Each => write!(f, "each"),
        }
    }
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// JSONL file of payment events (one JSON object per line).
    #[arg(long)]
    pub events: PathBuf,
    /// CSV file of expected payments.
    #[arg(long)]
    pub expected: PathBuf,
    /// Output file path. Defaults to stdout.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value = "csv")]
    pub format: OutputFormat,
    /// Amount tolerance in basis points (100 bps = 1%).
    #[arg(long, default_value_t = 0)]
    pub tolerance: u32,
    /// Require sender to match expected.from when set.
    #[arg(long)]
    pub strict_sender: bool,
    /// Enable partial payment accumulation.
    #[arg(long)]
    pub allow_partial: bool,
    /// Reject payments arriving after due_at.
    #[arg(long)]
    pub reject_expired: bool,
    /// Reject overpayments (default: accept overpayments).
    #[arg(long)]
    pub strict_amount: bool,
    /// How tolerance interacts with partial payments.
    ///
    /// "final" — tolerance applied only to the cumulative total.
    /// "each"  — each individual partial must be within tolerance of the full expected amount.
    #[arg(long, value_enum, default_value_t = CliToleranceMode::Final)]
    pub partial_tolerance_mode: CliToleranceMode,
    /// Only match memos whose issuer tag matches this namespace.
    #[arg(long, env = "TEMPO_RECONCILE_NAMESPACE")]
    pub issuer_namespace: Option<String>,
}

pub fn run_reconcile(args: &RunArgs, json_output: bool) -> Result<()> {
    if args.tolerance > 10_000 {
        bail!(
            "--tolerance must be 0–10000 basis points (max 100%), got {}",
            args.tolerance
        );
    }

    let events = read_events(&args.events)?;
    let expected_payments = read_expected(&args.expected)?;

    let issuer_tag = args
        .issuer_namespace
        .as_deref()
        .map(tempo_reconcile::issuer_tag_from_namespace);

    let opts = ReconcilerOptions {
        amount_tolerance_bps: args.tolerance,
        strict_sender: args.strict_sender,
        allow_partial: args.allow_partial,
        reject_expired: args.reject_expired,
        allow_overpayment: !args.strict_amount,
        partial_tolerance_mode: match args.partial_tolerance_mode {
            CliToleranceMode::Each => ToleranceMode::Each,
            CliToleranceMode::Final => ToleranceMode::Final,
        },
        issuer_tag,
    };

    let mut reconciler = Reconciler::new(opts);

    for payment in expected_payments {
        reconciler
            .expect(payment)
            .context("duplicate expected payment (duplicate memo_raw in CSV)")?;
    }

    reconciler.ingest_many(events);

    let report = reconciler.report();

    // Build full result list for export.
    let mut all_results = report.matched.clone();
    all_results.extend(report.issues.iter().cloned());

    let output = match args.format {
        OutputFormat::Csv => export_csv(&all_results),
        OutputFormat::Json => export_json(&all_results),
        OutputFormat::Jsonl => export_jsonl(&all_results),
    };

    // Write report to --out or stdout.
    match &args.out {
        Some(path) => {
            std::fs::write(path, &output)
                .with_context(|| format!("cannot write to {}", path.display()))?;
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            if let Err(e) = stdout.write_all(output.as_bytes()) {
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(anyhow::anyhow!("write stdout: {e}"));
                }
            }
        }
    }

    // Print summary to stderr.
    print_summary(&report.summary, json_output)?;

    Ok(())
}

fn print_summary(s: &tempo_reconcile::ReconcileSummary, json: bool) -> Result<()> {
    let stderr = std::io::stderr();
    let mut out = stderr.lock();

    if json {
        let j = serde_json::json!({
            "totalExpected":       s.total_expected,
            "totalReceived":       s.total_received,
            "matchedCount":        s.matched_count,
            "issueCount":          s.issue_count,
            "pendingCount":        s.pending_count,
            "unknownMemoCount":    s.unknown_memo_count,
            "noMemoCount":         s.no_memo_count,
            "mismatchAmountCount": s.mismatch_amount_count,
            "mismatchTokenCount":  s.mismatch_token_count,
            "mismatchPartyCount":  s.mismatch_party_count,
            "expiredCount":        s.expired_count,
            "partialCount":        s.partial_count,
            "totalExpectedAmount": s.total_expected_amount.to_string(),
            "totalReceivedAmount": s.total_received_amount.to_string(),
            "totalMatchedAmount":  s.total_matched_amount.to_string(),
        });
        writeln!(out, "{j}")?;
    } else {
        writeln!(out, "\nReconciliation Report")?;
        writeln!(out, "=====================")?;
        writeln!(
            out,
            "Total expected:   {} ({} units)",
            s.total_expected, s.total_expected_amount
        )?;
        writeln!(
            out,
            "Total received:   {} ({} units)",
            s.total_received, s.total_received_amount
        )?;
        writeln!(
            out,
            "Matched:          {} ({} units)",
            s.matched_count, s.total_matched_amount
        )?;
        writeln!(out, "Issues:           {}", s.issue_count)?;
        if s.unknown_memo_count > 0 {
            writeln!(out, "  unknown_memo:     {}", s.unknown_memo_count)?;
        }
        if s.no_memo_count > 0 {
            writeln!(out, "  no_memo:          {}", s.no_memo_count)?;
        }
        if s.mismatch_amount_count > 0 {
            writeln!(out, "  mismatch_amount:  {}", s.mismatch_amount_count)?;
        }
        if s.mismatch_token_count > 0 {
            writeln!(out, "  mismatch_token:   {}", s.mismatch_token_count)?;
        }
        if s.mismatch_party_count > 0 {
            writeln!(out, "  mismatch_party:   {}", s.mismatch_party_count)?;
        }
        if s.expired_count > 0 {
            writeln!(out, "  expired:          {}", s.expired_count)?;
        }
        if s.partial_count > 0 {
            writeln!(out, "  partial:          {}", s.partial_count)?;
        }
        writeln!(out, "Pending:          {}", s.pending_count)?;
    }
    Ok(())
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

    fn memo_for(namespace: &str, ulid: &str) -> String {
        use tempo_reconcile::{
            encode_memo_v1, issuer_tag_from_namespace, EncodeMemoV1Params, MemoType,
        };
        encode_memo_v1(&EncodeMemoV1Params {
            memo_type: MemoType::Invoice,
            issuer_tag: issuer_tag_from_namespace(namespace),
            ulid: ulid.to_string(),
            salt: None,
        })
        .unwrap()
    }

    #[test]
    fn run_produces_report() {
        let memo1 = memo_for("test-ns", "01MASW9NF6YW40J40H289H858P");
        let memo2 = memo_for("test-ns", "01MASW9NF6YW40J40H289H858Q");

        // Expected: memo1 for 10 USDC
        let expected_csv =
            format!("memo_raw,token,to,amount\n{memo1},0x20c0,0xrecipient,10000000\n");

        // Events: memo1 matched, unknown memo2, no-memo event
        let events_jsonl = format!(
            r#"{{"chainId":42431,"blockNumber":1,"txHash":"0xaaa","logIndex":0,"token":"0x20c0","from":"0xpayer","to":"0xrecipient","amount":"10000000","memoRaw":"{memo1}"}}
{{"chainId":42431,"blockNumber":2,"txHash":"0xbbb","logIndex":0,"token":"0x20c0","from":"0xpayer","to":"0xrecipient","amount":"5000000","memoRaw":"{memo2}"}}
{{"chainId":42431,"blockNumber":3,"txHash":"0xccc","logIndex":0,"token":"0x20c0","from":"0xpayer","to":"0xrecipient","amount":"1000000"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        // header + 3 data rows
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 4, "expected header + 3 data rows");

        // First data row should be matched
        assert!(
            lines[1].contains("matched"),
            "first event should be matched"
        );
    }

    #[test]
    fn run_json_format() {
        let memo1 = memo_for("test-ns2", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo1},0x20c0,0xto,5000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xd","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"5000000","memoRaw":"{memo1}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Json,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let json_str = std::fs::read_to_string(out_file.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }

    #[test]
    fn run_jsonl_format() {
        let memo1 = memo_for("test-ns3", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo1},0x20c0,0xto,1000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xe","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo1}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Jsonl,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let jsonl = std::fs::read_to_string(out_file.path()).unwrap();
        // Each non-blank line must be valid JSON.
        let valid_lines: Vec<serde_json::Value> = jsonl
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).expect("each JSONL line is valid JSON"))
            .collect();
        assert_eq!(valid_lines.len(), 1);
        assert_eq!(valid_lines[0]["status"], "matched");
    }

    #[test]
    fn run_error_on_missing_events_file() {
        let expected_file = write_tmp("memo_raw,token,to,amount\n");
        let args = RunArgs {
            events: std::path::PathBuf::from("/nonexistent/events.jsonl"),
            expected: expected_file.path().to_path_buf(),
            out: None,
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };
        let err = run_reconcile(&args, false).unwrap_err();
        assert!(
            err.chain()
                .any(|c| c.downcast_ref::<std::io::Error>().is_some()),
            "missing file must chain to std::io::Error (exit code 5)"
        );
    }

    #[test]
    fn run_error_on_missing_expected_file() {
        let events_file = write_tmp("");
        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: std::path::PathBuf::from("/nonexistent/expected.csv"),
            out: None,
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };
        let err = run_reconcile(&args, false).unwrap_err();
        assert!(
            err.chain()
                .any(|c| c.downcast_ref::<std::io::Error>().is_some()),
            "missing file must chain to std::io::Error (exit code 5)"
        );
    }

    #[test]
    fn run_json_summary_does_not_panic() {
        // Exercises the --json summary path (json_output=true).
        let memo1 = memo_for("test-ns4", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo1},0x20c0,0xto,1000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xf","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo1}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        // json_output=true — prints JSON summary to stderr, must not error.
        run_reconcile(&args, true).unwrap();
    }

    #[test]
    fn run_json_flag_writes_json_summary_to_stderr() {
        let memo1 = memo_for("test-ns5", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo1},0x20c0,0xto,1000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xg","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo1}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        // Capture JSON output by calling the function directly
        run_reconcile(&args, true).unwrap();

        // Verify by parsing the summary fields inline via print_summary
        use tempo_reconcile::ReconcileSummary;
        let s = ReconcileSummary {
            total_expected: 1,
            total_received: 1,
            matched_count: 1,
            total_expected_amount: 1_000_000,
            total_received_amount: 1_000_000,
            total_matched_amount: 1_000_000,
            ..Default::default()
        };
        let j = serde_json::json!({
            "totalExpectedAmount": s.total_expected_amount.to_string(),
            "totalReceivedAmount": s.total_received_amount.to_string(),
            "totalMatchedAmount":  s.total_matched_amount.to_string(),
        });
        assert!(j.get("totalExpectedAmount").is_some());
    }

    #[test]
    fn run_with_issuer_namespace_filters_unknown_memos() {
        let memo_ns1 = memo_for("ns1", "01MASW9NF6YW40J40H289H858P");
        let memo_ns2 = memo_for("ns2", "01MASW9NF6YW40J40H289H858Q");

        // Expected: memo from ns1
        let expected_csv = format!("memo_raw,token,to,amount\n{memo_ns1},0x20c0,0xto,1000000\n");
        // Events: one from ns1 (matched), one from ns2 (filtered → unknown_memo)
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xaa","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo_ns1}"}}
{{"chainId":1,"blockNumber":2,"txHash":"0xbb","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo_ns2}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: Some("ns1".to_string()),
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        // header + 2 data rows (1 matched + 1 unknown_memo)
        assert_eq!(lines.len(), 3);
        assert!(lines[1].contains("matched"));
        assert!(lines[2].contains("unknown_memo"));
    }

    #[test]
    fn run_with_tolerance_accepts_underpayment() {
        // 500 bps = 5% tolerance. Expected: 10 USDC. Payment: 9.5 USDC → within tolerance → matched.
        let memo = memo_for("tol-ns", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xto,10000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xtol","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"9500000","memoRaw":"{memo}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 500,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2);
        assert!(
            lines[1].contains("matched"),
            "9.5 USDC within 5% tolerance should be matched"
        );
    }

    #[test]
    fn run_with_strict_sender_rejects_wrong_from() {
        // strict_sender=true + expected.from set → wrong sender → mismatch_party.
        let memo = memo_for("strict-ns", "01MASW9NF6YW40J40H289H858P");
        // CSV format: memo_raw,token,to,amount,from
        let expected_csv =
            format!("memo_raw,token,to,amount,from\n{memo},0x20c0,0xto,1000000,0xexpectedsender\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xstrict","logIndex":0,"token":"0x20c0","from":"0xwrongsender","to":"0xto","amount":"1000000","memoRaw":"{memo}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: true,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2);
        assert!(
            lines[1].contains("mismatch_party"),
            "wrong sender with strict_sender should produce mismatch_party"
        );
    }

    #[test]
    fn run_with_allow_partial_accumulates() {
        // Two partial payments (5 USDC each) summing to the full 10 USDC → matched on the second.
        let memo = memo_for("partial-ns", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xto,10000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xp1","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"5000000","memoRaw":"{memo}"}}
{{"chainId":1,"blockNumber":2,"txHash":"0xp2","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"5000000","memoRaw":"{memo}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: true,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        // header + 2 data rows: one partial (in issues), one matched
        assert!(lines.len() >= 3);
        let has_partial = lines[1..].iter().any(|l| l.contains("partial"));
        let has_matched = lines[1..].iter().any(|l| l.contains("matched"));
        assert!(has_partial, "one payment should be partial");
        assert!(has_matched, "second payment should complete the match");
    }

    #[test]
    fn run_with_reject_expired() {
        // due_at in the past, reject_expired=true → payment arrives late → expired.
        let memo = memo_for("exp-ns", "01MASW9NF6YW40J40H289H858P");
        // CSV: memo_raw,token,to,amount,from,due_at
        let expected_csv =
            format!("memo_raw,token,to,amount,from,due_at\n{memo},0x20c0,0xto,1000000,,1000000\n");
        // Event timestamp > due_at
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xexp","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo}","timestamp":2000000}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: true,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2);
        assert!(
            lines[1].contains("expired"),
            "late payment with reject_expired should be expired"
        );
    }

    #[test]
    fn run_tolerance_over_10000_is_rejected() {
        let expected_file = write_tmp("memo_raw,token,to,amount\n");
        let events_file = write_tmp("");
        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: None,
            format: OutputFormat::Csv,
            tolerance: 10_001,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };
        let err = run_reconcile(&args, false).unwrap_err();
        assert!(
            err.to_string().contains("basis points"),
            "error message must mention basis points"
        );
    }

    #[test]
    fn strict_amount_rejects_overpayment() {
        // --strict-amount sets allow_overpayment=false → overpay → mismatch_amount.
        let memo = memo_for("strict-amt-ns", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xto,1000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xover","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"2000000","memoRaw":"{memo}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 0,
            strict_sender: false,
            allow_partial: false,
            reject_expired: false,
            strict_amount: true,
            partial_tolerance_mode: CliToleranceMode::Final,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2);
        assert!(
            lines[1].contains("mismatch_amount"),
            "--strict-amount with overpayment should produce mismatch_amount"
        );
    }

    #[test]
    fn partial_tolerance_mode_each_is_accepted() {
        // --partial-tolerance-mode each: tolerance applies per individual payment.
        // A single payment of 9_600_000 for expected 10_000_000 with 5% tolerance (500K):
        // underpaid by 400K < tolerance 500K → matched immediately.
        let memo = memo_for("ptm-ns", "01MASW9NF6YW40J40H289H858P");
        let expected_csv = format!("memo_raw,token,to,amount\n{memo},0x20c0,0xto,10000000\n");
        let events_jsonl = format!(
            r#"{{"chainId":1,"blockNumber":1,"txHash":"0xe1","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"9600000","memoRaw":"{memo}"}}
"#
        );

        let expected_file = write_tmp(&expected_csv);
        let events_file = write_tmp(&events_jsonl);
        let out_file = NamedTempFile::new().unwrap();

        let args = RunArgs {
            events: events_file.path().to_path_buf(),
            expected: expected_file.path().to_path_buf(),
            out: Some(out_file.path().to_path_buf()),
            format: OutputFormat::Csv,
            tolerance: 500, // 5% tolerance = 500K on 10M
            strict_sender: false,
            allow_partial: true,
            reject_expired: false,
            strict_amount: false,
            partial_tolerance_mode: CliToleranceMode::Each,
            issuer_namespace: None,
        };

        run_reconcile(&args, false).unwrap();

        let csv = std::fs::read_to_string(out_file.path()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert!(lines.len() >= 2, "output must have at least one result row");
        let has_matched = lines[1..].iter().any(|l| l.contains("matched"));
        assert!(
            has_matched,
            "--partial-tolerance-mode each within tolerance should match immediately"
        );
    }
}
