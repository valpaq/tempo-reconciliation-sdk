use tempo_reconcile::{encode_memo_v1, EncodeMemoV1Params, ExpectedPayment, PaymentEvent};

pub use tempo_reconcile::issuer_tag_from_namespace;
pub use tempo_reconcile::MemoType;

#[allow(dead_code)]
pub const ULID_A: &str = "01MASW9NF6YW40J40H289H858P";
#[allow(dead_code)]
pub const ULID_B: &str = "01MASW9NF6YW40J40H289H8580";

pub fn make_memo(memo_type: MemoType, ulid: &str) -> String {
    encode_memo_v1(&EncodeMemoV1Params {
        memo_type,
        issuer_tag: issuer_tag_from_namespace("test-ns"),
        ulid: ulid.to_string(),
        salt: None,
    })
    .unwrap()
}

pub fn make_event(memo_raw: Option<&str>, amount: u128) -> PaymentEvent {
    PaymentEvent {
        chain_id: 42431,
        block_number: 1,
        tx_hash: "0xdeadbeef".to_string(),
        log_index: 0,
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        from: "0xsender".to_string(),
        to: "0xrecipient".to_string(),
        amount,
        memo_raw: memo_raw.map(|s| s.to_string()),
        memo: None,
        timestamp: None,
    }
}

pub fn make_expected(memo_raw: &str, amount: u128) -> ExpectedPayment {
    ExpectedPayment {
        memo_raw: memo_raw.to_string(),
        token: "0x20c0000000000000000000000000000000000000".to_string(),
        to: "0xrecipient".to_string(),
        amount,
        from: None,
        due_at: None,
        meta: None,
    }
}
