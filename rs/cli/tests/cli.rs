/// Binary integration tests for the `tempo-reconcile` CLI.
///
/// These tests invoke the compiled binary as a subprocess, verifying
/// argument parsing, exit codes, and output that inline unit tests cannot cover.
use std::io::Write as _;
use std::process::Command;

use tempfile::NamedTempFile;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tempo-reconcile"))
}

fn write_tmp(content: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f
}

/// Run `memo encode` for the given namespace + ULID; return the trimmed hex output.
fn encode_via_cli(namespace: &str, ulid: &str) -> String {
    let out = cli()
        .args([
            "memo",
            "encode",
            "--type",
            "invoice",
            "--namespace",
            namespace,
            "--ulid",
            ulid,
        ])
        .output()
        .expect("failed to spawn tempo-reconcile");
    assert!(
        out.status.success(),
        "memo encode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

// ---------------------------------------------------------------------------
// Help / version
// ---------------------------------------------------------------------------

#[test]
fn help_exits_zero() {
    let status = cli().arg("--help").status().unwrap();
    assert!(status.success());
}

#[test]
fn version_exits_zero() {
    let status = cli().arg("--version").status().unwrap();
    assert!(status.success());
}

#[test]
fn no_subcommand_exits_nonzero() {
    // clap exits with code 2 when no subcommand is given.
    let status = cli().status().unwrap();
    assert!(!status.success());
}

// ---------------------------------------------------------------------------
// memo encode
// ---------------------------------------------------------------------------

#[test]
fn memo_encode_produces_66_char_hex() {
    let memo = encode_via_cli("tempo-reconcile", "01MASW9NF6YW40J40H289H858P");
    assert!(memo.starts_with("0x"), "must start with 0x");
    assert_eq!(memo.len(), 66, "must be 0x + 64 hex chars");
}

#[test]
fn memo_encode_json_flag_has_memo_raw_key() {
    let out = cli()
        .args([
            "--json",
            "memo",
            "encode",
            "--type",
            "invoice",
            "--namespace",
            "acme",
            "--ulid",
            "01MASW9NF6YW40J40H289H858P",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(parsed.get("memoRaw").is_some());
    let memo = parsed["memoRaw"].as_str().unwrap();
    assert!(memo.starts_with("0x"));
    assert_eq!(memo.len(), 66);
}

#[test]
fn memo_encode_empty_namespace_exits_nonzero() {
    let status = cli()
        .args([
            "memo",
            "encode",
            "--type",
            "invoice",
            "--namespace",
            "",
            "--ulid",
            "01MASW9NF6YW40J40H289H858P",
        ])
        .status()
        .unwrap();
    assert!(!status.success());
}

// ---------------------------------------------------------------------------
// memo decode
// ---------------------------------------------------------------------------

#[test]
fn memo_decode_spec_vector() {
    // spec vector: invoice, namespace "tempo-reconcile"
    let memo_raw = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let out = cli().args(["memo", "decode", memo_raw]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("invoice"), "output must mention type");
    assert!(
        stdout.contains("fc7c8482914a04e8"),
        "output must contain issuer tag"
    );
}

#[test]
fn memo_decode_json_flag_has_expected_keys() {
    let memo_raw = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
    let out = cli()
        .args(["--json", "memo", "decode", memo_raw])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["type"], "invoice");
    assert!(parsed.get("issuerTag").is_some());
    assert!(parsed.get("ulid").is_some());
    assert!(parsed.get("salt").is_some());
}

#[test]
fn memo_decode_all_zeros_exits_nonzero() {
    let all_zeros = "0x0000000000000000000000000000000000000000000000000000000000000000";
    let status = cli().args(["memo", "decode", all_zeros]).status().unwrap();
    assert!(!status.success());
}

// ---------------------------------------------------------------------------
// memo generate
// ---------------------------------------------------------------------------

#[test]
fn memo_generate_output_is_decodable() {
    // Generate a memo, then decode it — must succeed.
    let gen_out = cli()
        .args([
            "--json",
            "memo",
            "generate",
            "--type",
            "invoice",
            "--namespace",
            "my-app",
        ])
        .output()
        .unwrap();
    assert!(gen_out.status.success());
    let stdout = String::from_utf8_lossy(&gen_out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    let memo_raw = parsed["memoRaw"].as_str().unwrap();

    let dec_status = cli().args(["memo", "decode", memo_raw]).status().unwrap();
    assert!(dec_status.success(), "generated memo must be decodable");
}

// ---------------------------------------------------------------------------
// memo issuer-tag
// ---------------------------------------------------------------------------

#[test]
fn memo_issuer_tag_spec_vector() {
    // keccak256("tempo-reconcile")[0:8] = 0xfc7c8482914a04e8
    let out = cli()
        .args(["memo", "issuer-tag", "tempo-reconcile"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fc7c8482914a04e8"),
        "issuer tag must match spec: got {stdout}"
    );
}

#[test]
fn memo_issuer_tag_json_flag() {
    let out = cli()
        .args(["--json", "memo", "issuer-tag", "tempo-reconcile"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["namespace"], "tempo-reconcile");
    assert!(parsed.get("issuerTag").is_some());
    assert!(parsed.get("issuerTagDecimal").is_some());
}

// ---------------------------------------------------------------------------
// encode → decode roundtrip via subprocess
// ---------------------------------------------------------------------------

#[test]
fn memo_encode_decode_roundtrip_via_subprocess() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let namespace = "roundtrip-ns";

    let memo_raw = encode_via_cli(namespace, ulid);

    let dec_out = cli()
        .args(["--json", "memo", "decode", &memo_raw])
        .output()
        .unwrap();
    assert!(dec_out.status.success());
    let stdout = String::from_utf8_lossy(&dec_out.stdout);
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(parsed["type"], "invoice");
    assert_eq!(parsed["ulid"], ulid);
}

// ---------------------------------------------------------------------------
// run command
// ---------------------------------------------------------------------------

#[test]
fn run_empty_inputs_exits_zero() {
    let events = write_tmp("");
    let expected = write_tmp("memo_raw,token,to,amount\n");
    let out_file = NamedTempFile::new().unwrap();

    let status = cli()
        .args([
            "run",
            "--events",
            events.path().to_str().unwrap(),
            "--expected",
            expected.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn run_missing_events_file_exits_5() {
    let expected = write_tmp("memo_raw,token,to,amount\n");

    let output = cli()
        .args([
            "run",
            "--events",
            "/nonexistent/events.jsonl",
            "--expected",
            expected.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(5),
        "missing events file must exit 5"
    );
}

#[test]
fn run_missing_expected_file_exits_5() {
    let events = write_tmp("");

    let output = cli()
        .args([
            "run",
            "--events",
            events.path().to_str().unwrap(),
            "--expected",
            "/nonexistent/expected.csv",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(5),
        "missing expected file must exit 5"
    );
}

#[test]
fn run_tolerance_over_limit_exits_1() {
    let events = write_tmp("");
    let expected = write_tmp("memo_raw,token,to,amount\n");

    let output = cli()
        .args([
            "run",
            "--events",
            events.path().to_str().unwrap(),
            "--expected",
            expected.path().to_str().unwrap(),
            "--tolerance",
            "10001",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "tolerance > 10000 must exit 1"
    );
}

#[test]
fn run_matched_payment_appears_in_csv_output() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let memo_raw = encode_via_cli("pipe-ns", ulid);

    let expected_csv = format!("memo_raw,token,to,amount\n{memo_raw},0x20c0,0xto,5000000\n");
    let events_jsonl = format!(
        r#"{{"chainId":42431,"blockNumber":1,"txHash":"0xabc","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"5000000","memoRaw":"{memo_raw}"}}
"#
    );

    let expected_file = write_tmp(&expected_csv);
    let events_file = write_tmp(&events_jsonl);
    let out_file = NamedTempFile::new().unwrap();

    let status = cli()
        .args([
            "run",
            "--events",
            events_file.path().to_str().unwrap(),
            "--expected",
            expected_file.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let csv = std::fs::read_to_string(out_file.path()).unwrap();
    assert!(
        csv.contains("matched"),
        "CSV output must contain matched row"
    );
}

#[test]
fn run_json_flag_writes_json_summary_to_stderr() {
    let events = write_tmp("");
    let expected = write_tmp("memo_raw,token,to,amount\n");
    let out_file = NamedTempFile::new().unwrap();

    let output = cli()
        .args([
            "--json",
            "run",
            "--events",
            events.path().to_str().unwrap(),
            "--expected",
            expected.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let parsed: serde_json::Value =
        serde_json::from_str(stderr.trim()).expect("--json flag must write JSON summary to stderr");
    assert!(parsed.get("matchedCount").is_some());
    assert!(parsed.get("pendingCount").is_some());
}

#[test]
fn run_format_json_output_is_array() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let memo_raw = encode_via_cli("fmt-ns", ulid);

    let expected_csv = format!("memo_raw,token,to,amount\n{memo_raw},0x20c0,0xto,1000000\n");
    let events_jsonl = format!(
        r#"{{"chainId":1,"blockNumber":1,"txHash":"0xf1","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo_raw}"}}
"#
    );

    let expected_file = write_tmp(&expected_csv);
    let events_file = write_tmp(&events_jsonl);
    let out_file = NamedTempFile::new().unwrap();

    let status = cli()
        .args([
            "run",
            "--events",
            events_file.path().to_str().unwrap(),
            "--expected",
            expected_file.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(out_file.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.is_array(), "--format json must produce a JSON array");
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn run_format_jsonl_each_line_is_valid_json() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let memo_raw = encode_via_cli("jsonl-ns", ulid);

    let expected_csv = format!("memo_raw,token,to,amount\n{memo_raw},0x20c0,0xto,1000000\n");
    let events_jsonl = format!(
        r#"{{"chainId":1,"blockNumber":1,"txHash":"0xf2","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo_raw}"}}
"#
    );

    let expected_file = write_tmp(&expected_csv);
    let events_file = write_tmp(&events_jsonl);
    let out_file = NamedTempFile::new().unwrap();

    let status = cli()
        .args([
            "run",
            "--events",
            events_file.path().to_str().unwrap(),
            "--expected",
            expected_file.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
            "--format",
            "jsonl",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(out_file.path()).unwrap();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|_| panic!("JSONL line is not valid JSON: {line}"));
    }
}

#[test]
fn run_format_csv_output_has_csv_header() {
    let ulid = "01MASW9NF6YW40J40H289H858P";
    let memo_raw = encode_via_cli("csv-ns", ulid);

    let expected_csv = format!("memo_raw,token,to,amount\n{memo_raw},0x20c0,0xto,1000000\n");
    let events_jsonl = format!(
        r#"{{"chainId":1,"blockNumber":1,"txHash":"0xf3","logIndex":0,"token":"0x20c0","from":"0xf","to":"0xto","amount":"1000000","memoRaw":"{memo_raw}"}}
"#
    );

    let expected_file = write_tmp(&expected_csv);
    let events_file = write_tmp(&events_jsonl);
    let out_file = NamedTempFile::new().unwrap();

    let status = cli()
        .args([
            "run",
            "--events",
            events_file.path().to_str().unwrap(),
            "--expected",
            expected_file.path().to_str().unwrap(),
            "--out",
            out_file.path().to_str().unwrap(),
            "--format",
            "csv",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(out_file.path()).unwrap();
    let first_line = content.lines().next().unwrap_or("");
    assert!(
        first_line.contains("status") || first_line.contains("memo_raw"),
        "--format csv must produce a CSV header, got: {first_line}"
    );
}
