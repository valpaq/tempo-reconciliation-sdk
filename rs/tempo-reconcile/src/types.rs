use std::collections::HashMap;

/// Payment type encoded in byte 0 of the memo.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MemoType {
    Invoice,
    Payroll,
    Refund,
    Batch,
    Subscription,
    Custom,
}

impl MemoType {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoType::Invoice => "invoice",
            MemoType::Payroll => "payroll",
            MemoType::Refund => "refund",
            MemoType::Batch => "batch",
            MemoType::Subscription => "subscription",
            MemoType::Custom => "custom",
        }
    }

    /// Returns the wire byte for this memo type.
    #[must_use]
    pub fn type_byte(&self) -> u8 {
        match self {
            MemoType::Invoice => 0x01,
            MemoType::Payroll => 0x02,
            MemoType::Refund => 0x03,
            MemoType::Batch => 0x04,
            MemoType::Subscription => 0x05,
            MemoType::Custom => 0x0F,
        }
    }
}

impl std::fmt::Display for MemoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Decoded v1 structured memo (TEMPO-RECONCILE-MEMO-001).
///
/// Layout: `[type:1][issuerTag:8][id16:16][salt:7]` = 32 bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoV1 {
    /// Memo protocol version, always `1` for v1 memos.
    pub v: u8,
    /// Payment type.
    pub t: MemoType,
    /// Issuer namespace tag: first 8 bytes of keccak256(namespace), as u64 big-endian.
    pub issuer_tag: u64,
    /// ULID reference, 26-char Crockford base32 string.
    pub ulid: String,
    /// ULID in 16-byte binary form.
    pub id16: [u8; 16],
    /// Optional salt / metadata (7 bytes, default zeros).
    pub salt: [u8; 7],
    /// Original bytes32 as "0x" + 64 lowercase hex chars.
    pub raw: String,
}

/// A decoded memo: either a v1 structured memo or a plain UTF-8 text string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Memo {
    V1(MemoV1),
    Text(String),
}

/// A TIP-20 transfer event observed on-chain.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PaymentEvent {
    pub chain_id: u32,
    pub block_number: u64,
    /// Transaction hash: "0x" + 64 lowercase hex chars.
    pub tx_hash: String,
    pub log_index: u32,
    /// Token contract address: "0x" + 40 lowercase hex chars.
    pub token: String,
    /// Sender address.
    pub from: String,
    /// Recipient address.
    pub to: String,
    /// Raw token amount in smallest unit (6 decimals for TIP-20).
    pub amount: u128,
    /// Raw bytes32 memo hex, if the event was TransferWithMemo.
    pub memo_raw: Option<String>,
    /// Decoded memo, if present.
    pub memo: Option<Memo>,
    /// Block timestamp in Unix seconds, if fetched.
    pub timestamp: Option<u64>,
}

/// An expected payment registered with the Reconciler.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExpectedPayment {
    /// Primary key — bytes32 memo hex ("0x" + 64 hex chars).
    pub memo_raw: String,
    /// Expected token contract address.
    pub token: String,
    /// Expected recipient address.
    pub to: String,
    /// Expected amount in smallest unit.
    pub amount: u128,
    /// Optional sender constraint (used if strictSender=true).
    pub from: Option<String>,
    /// Optional payment deadline, Unix seconds.
    pub due_at: Option<u64>,
    /// Arbitrary business metadata (invoice ID, customer, etc.).
    pub meta: Option<HashMap<String, String>>,
}

/// Result of matching one PaymentEvent against registered expectations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum MatchStatus {
    /// Memo found, amount within tolerance, all constraints pass.
    Matched,
    /// Underpayment accumulated (allowPartial=true).
    Partial,
    /// Memo present but not in expected list (or issuerTag filtered out).
    UnknownMemo,
    /// Transfer without memo field.
    NoMemo,
    /// Memo found but amount differs.
    MismatchAmount,
    /// Memo found but wrong token contract.
    MismatchToken,
    /// Memo found but sender/recipient mismatch.
    MismatchParty,
    /// Payment arrived after due_at (rejectExpired=true).
    Expired,
}

impl MatchStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            MatchStatus::Matched => "matched",
            MatchStatus::Partial => "partial",
            MatchStatus::UnknownMemo => "unknown_memo",
            MatchStatus::NoMemo => "no_memo",
            MatchStatus::MismatchAmount => "mismatch_amount",
            MatchStatus::MismatchToken => "mismatch_token",
            MatchStatus::MismatchParty => "mismatch_party",
            MatchStatus::Expired => "expired",
        }
    }
}

impl std::fmt::Display for MatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Result of processing one PaymentEvent through the Reconciler.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchResult {
    pub status: MatchStatus,
    pub payment: PaymentEvent,
    /// The matching expected payment, if found.
    pub expected: Option<ExpectedPayment>,
    /// Human-readable reason for non-matched statuses.
    pub reason: Option<String>,
    /// Set when amount > expected (status = Matched with overpayment).
    pub overpaid_by: Option<u128>,
    /// Set for partial payments: how much is still outstanding.
    pub remaining_amount: Option<u128>,
    /// Set when payment arrived after due_at.
    pub is_late: Option<bool>,
}

/// Full reconciliation report.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReconcileReport {
    pub matched: Vec<MatchResult>,
    pub issues: Vec<MatchResult>,
    pub pending: Vec<ExpectedPayment>,
    pub summary: ReconcileSummary,
}

/// Aggregate statistics for a ReconcileReport.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReconcileSummary {
    pub total_expected: usize,
    pub total_received: usize,
    pub matched_count: usize,
    pub issue_count: usize,
    pub pending_count: usize,
    pub total_expected_amount: u128,
    pub total_received_amount: u128,
    pub total_matched_amount: u128,
    pub unknown_memo_count: usize,
    pub no_memo_count: usize,
    pub mismatch_amount_count: usize,
    pub mismatch_token_count: usize,
    pub mismatch_party_count: usize,
    pub expired_count: usize,
    pub partial_count: usize,
}
