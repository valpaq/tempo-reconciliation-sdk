use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use tempo_reconcile::{
    decode_memo_v1, encode_memo_v1, issuer_tag_from_namespace, random_salt, EncodeMemoV1Params,
    MemoType,
};

/// Memo type accepted on the command line.
#[derive(Clone, ValueEnum, Debug)]
pub enum CliMemoType {
    Invoice,
    Payroll,
    Refund,
    Batch,
    Subscription,
    Custom,
}

impl CliMemoType {
    fn as_str(&self) -> &'static str {
        MemoType::from(self.clone()).as_str()
    }
}

impl From<CliMemoType> for MemoType {
    fn from(t: CliMemoType) -> Self {
        match t {
            CliMemoType::Invoice => MemoType::Invoice,
            CliMemoType::Payroll => MemoType::Payroll,
            CliMemoType::Refund => MemoType::Refund,
            CliMemoType::Batch => MemoType::Batch,
            CliMemoType::Subscription => MemoType::Subscription,
            CliMemoType::Custom => MemoType::Custom,
        }
    }
}

#[derive(Args, Debug)]
pub struct MemoArgs {
    #[command(subcommand)]
    pub command: MemoCommand,
}

#[derive(Subcommand, Debug)]
pub enum MemoCommand {
    /// Encode a memo from its components.
    Encode(EncodeArgs),
    /// Decode a bytes32 memo.
    Decode(DecodeArgs),
    /// Generate a new ULID and encode a memo.
    Generate(GenerateArgs),
    /// Compute the issuer tag for a namespace string.
    IssuerTag(IssuerTagArgs),
}

#[derive(Args, Debug)]
pub struct EncodeArgs {
    /// Payment type.
    #[arg(long, value_enum)]
    pub r#type: CliMemoType,
    /// Namespace string used to derive the issuer tag.
    #[arg(long, env = "TEMPO_RECONCILE_NAMESPACE")]
    pub namespace: String,
    /// ULID string (26 Crockford base32 characters).
    #[arg(long)]
    pub ulid: String,
    /// Optional 7-byte salt as lowercase hex (14 hex chars). Defaults to zeros.
    #[arg(long)]
    pub salt: Option<String>,
}

#[derive(Args, Debug)]
pub struct DecodeArgs {
    /// The bytes32 memo as "0x" + 64 hex chars.
    pub memo_raw: String,
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Payment type.
    #[arg(long, value_enum)]
    pub r#type: CliMemoType,
    /// Namespace string used to derive the issuer tag.
    #[arg(long, env = "TEMPO_RECONCILE_NAMESPACE")]
    pub namespace: String,
    /// Use random salt instead of zeros.
    #[arg(long)]
    pub random_salt: bool,
}

#[derive(Args, Debug)]
pub struct IssuerTagArgs {
    /// Namespace string.
    pub namespace: String,
}

/// Validate ULID string at the CLI boundary: 26 Crockford base32 characters.
fn validate_ulid(ulid: &str) -> Result<()> {
    if ulid.len() != 26 {
        bail!(
            "--ulid must be exactly 26 characters (Crockford base32), got {}",
            ulid.len()
        );
    }
    for (i, ch) in ulid.chars().enumerate() {
        let upper = ch.to_ascii_uppercase();
        if !matches!(upper,
            '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z'
        ) {
            bail!(
                "--ulid contains invalid Crockford base32 character '{}' at position {}",
                ch,
                i
            );
        }
    }
    Ok(())
}

fn encode_output(args: &EncodeArgs, json: bool) -> Result<String> {
    if args.namespace.is_empty() {
        bail!("--namespace must not be empty");
    }
    validate_ulid(&args.ulid)?;
    let salt = parse_salt_opt(args.salt.as_deref())?;
    let issuer_tag = issuer_tag_from_namespace(&args.namespace);
    let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: args.r#type.clone().into(),
        issuer_tag,
        ulid: args.ulid.clone(),
        salt,
    })
    .context("failed to encode memo")?;

    if json {
        Ok(serde_json::json!({ "memoRaw": memo_raw }).to_string())
    } else {
        Ok(memo_raw)
    }
}

pub fn run_encode(args: &EncodeArgs, json: bool) -> Result<()> {
    println!("{}", encode_output(args, json)?);
    Ok(())
}

fn decode_output(args: &DecodeArgs, json: bool) -> Result<String> {
    let memo = decode_memo_v1(&args.memo_raw)
        .ok_or_else(|| anyhow::anyhow!("not a valid v1 memo: {}", args.memo_raw))?;

    if json {
        Ok(serde_json::json!({
            "type":      memo.t.as_str(),
            "issuerTag": format!("0x{:016x}", memo.issuer_tag),
            "ulid":      memo.ulid,
            "salt":      hex::encode(memo.salt),
        })
        .to_string())
    } else {
        Ok(format!(
            "Type:       {} (0x{:02x})\nIssuerTag:  0x{:016x}\nULID:       {}\nSalt:       {}",
            memo.t.as_str(),
            memo.t.type_byte(),
            memo.issuer_tag,
            memo.ulid,
            hex::encode(memo.salt),
        ))
    }
}

pub fn run_decode(args: &DecodeArgs, json: bool) -> Result<()> {
    println!("{}", decode_output(args, json)?);
    Ok(())
}

pub fn run_generate(args: &GenerateArgs, json: bool) -> Result<()> {
    if args.namespace.is_empty() {
        bail!("--namespace must not be empty");
    }
    let new_ulid = ulid::Ulid::new().to_string();
    let issuer_tag = issuer_tag_from_namespace(&args.namespace);
    let salt = if args.random_salt {
        Some(random_salt())
    } else {
        None
    };
    let memo_raw = encode_memo_v1(&EncodeMemoV1Params {
        memo_type: args.r#type.clone().into(),
        issuer_tag,
        ulid: new_ulid.clone(),
        salt,
    })
    .context("failed to encode memo")?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "ulid":    new_ulid,
                "memoRaw": memo_raw,
                "type":    args.r#type.as_str(),
                "issuerTag": format!("0x{:016x}", issuer_tag),
            })
        );
    } else {
        println!("ULID:  {new_ulid}");
        println!("Memo:  {memo_raw}");
    }
    Ok(())
}

pub fn run_issuer_tag(args: &IssuerTagArgs, json: bool) -> Result<()> {
    if args.namespace.is_empty() {
        bail!("--namespace must not be empty");
    }
    let tag = issuer_tag_from_namespace(&args.namespace);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "namespace": args.namespace,
                "issuerTag": format!("0x{:016x}", tag),
                "issuerTagDecimal": tag,
            })
        );
    } else {
        println!("0x{:016x} ({tag})", tag);
    }
    Ok(())
}

fn parse_salt_opt(salt_hex: Option<&str>) -> Result<Option<[u8; 7]>> {
    match salt_hex {
        None => Ok(None),
        Some(s) => {
            let bytes =
                hex::decode(s).with_context(|| format!("salt must be valid hex, got: {s:?}"))?;
            if bytes.len() != 7 {
                bail!(
                    "salt must be exactly 7 bytes (14 hex chars), got {} bytes",
                    bytes.len()
                );
            }
            let mut arr = [0u8; 7];
            arr.copy_from_slice(&bytes);
            Ok(Some(arr))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let encode_args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "tempo-reconcile".to_string(),
            ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
            salt: None,
        };
        run_encode(&encode_args, false).unwrap();
    }

    #[test]
    fn issuer_tag_matches_spec_vector() {
        let args = IssuerTagArgs {
            namespace: "tempo-reconcile".to_string(),
        };
        let tag = issuer_tag_from_namespace(&args.namespace);
        // spec: keccak256("tempo-reconcile")[0:8] = 0xfc7c8482914a04e8
        assert_eq!(tag, 0xfc7c8482914a04e8u64);
    }

    #[test]
    fn decode_invalid_returns_error() {
        let args = DecodeArgs {
            memo_raw: "0x0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        };
        assert!(run_decode(&args, false).is_err());
    }

    #[test]
    fn parse_salt_opt_valid() {
        let salt = parse_salt_opt(Some("ff010203040506")).unwrap().unwrap();
        assert_eq!(salt, [0xff, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    }

    #[test]
    fn parse_salt_opt_wrong_length_errors() {
        assert!(parse_salt_opt(Some("aabb")).is_err()); // 2 bytes
    }

    #[test]
    fn generate_produces_valid_memo() {
        let args = GenerateArgs {
            r#type: CliMemoType::Invoice,
            namespace: "my-app".to_string(),
            random_salt: false,
        };
        run_generate(&args, false).unwrap();
    }

    #[test]
    fn encode_empty_namespace_errors() {
        let args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "".to_string(),
            ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
            salt: None,
        };
        let err = run_encode(&args, false).unwrap_err();
        assert!(err.to_string().contains("--namespace must not be empty"));
    }

    #[test]
    fn generate_empty_namespace_errors() {
        let args = GenerateArgs {
            r#type: CliMemoType::Invoice,
            namespace: "".to_string(),
            random_salt: false,
        };
        let err = run_generate(&args, false).unwrap_err();
        assert!(err.to_string().contains("--namespace must not be empty"));
    }

    #[test]
    fn issuer_tag_empty_namespace_errors() {
        let args = IssuerTagArgs {
            namespace: "".to_string(),
        };
        let err = run_issuer_tag(&args, false).unwrap_err();
        assert!(err.to_string().contains("--namespace must not be empty"));
    }

    #[test]
    fn encode_invalid_ulid_length_errors() {
        let args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "test".to_string(),
            ulid: "TOOSHORT".to_string(),
            salt: None,
        };
        let err = run_encode(&args, false).unwrap_err();
        assert!(err.to_string().contains("26 characters"));
    }

    #[test]
    fn encode_invalid_ulid_chars_errors() {
        let args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "test".to_string(),
            ulid: "01MASW9NF6YW40J40H289H85U!".to_string(), // '!' is invalid
            salt: None,
        };
        let err = run_encode(&args, false).unwrap_err();
        assert!(err.to_string().contains("invalid Crockford"));
    }

    #[test]
    fn encode_json_output_has_memo_raw_key() {
        let args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "tempo-reconcile".to_string(),
            ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
            salt: None,
        };
        let out = encode_output(&args, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(parsed.get("memoRaw").is_some(), "memoRaw key missing");
        let memo_raw = parsed["memoRaw"].as_str().unwrap();
        assert!(memo_raw.starts_with("0x"), "memoRaw must start with 0x");
        assert_eq!(memo_raw.len(), 66); // "0x" + 64 hex chars
    }

    #[test]
    fn encode_plain_output_is_bare_hex() {
        let args = EncodeArgs {
            r#type: CliMemoType::Invoice,
            namespace: "tempo-reconcile".to_string(),
            ulid: "01MASW9NF6YW40J40H289H858P".to_string(),
            salt: None,
        };
        let out = encode_output(&args, false).unwrap();
        assert!(out.starts_with("0x"));
        assert_eq!(out.len(), 66);
    }

    #[test]
    fn decode_json_output_has_expected_keys() {
        // spec vector: invoice with default salt
        let memo_raw = "0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000";
        let args = DecodeArgs {
            memo_raw: memo_raw.to_string(),
        };
        let out = decode_output(&args, true).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["type"], "invoice");
        assert!(parsed.get("issuerTag").is_some());
        assert!(parsed.get("ulid").is_some());
        assert!(parsed.get("salt").is_some());
    }
}
