# TEMPO-RECONCILE-MEMO-001

## Memo standard for TIP-20 payment reconciliation

**Status:** Stable v1
**Created:** 2026-02-27
**Authors:** tempo-reconcile contributors
**License:** CC0-1.0 (public domain dedication)

---

## Summary

This document defines a structured `bytes32` layout for the `memo` parameter of TIP-20 `transferWithMemo(address, uint256, bytes32)`. The goal is to provide a standard, interoperable way for applications to attach payment references (invoice IDs, payroll references, batch identifiers) to on-chain transfers without storing PII.

## Scope

MEMO-001 is designed for the fixed `bytes32` constraint of TIP-20. If Tempo introduces variable-length memos (e.g. `transferWithMemoExtended(address, uint256, bytes)`), a separate standard should be developed. This spec will remain valid for all existing `bytes32` transfers.

---

## Why

TIP-20 provides a native `bytes32 memo` field on transfers. Without a shared convention for how to use those 32 bytes, every team invents its own format. This leads to:

- **No interoperability** between apps (marketplace can't read payroll app's memos)
- **No tooling** (parsers/watchers have to handle N formats)
- **PII leaks** (teams naively encode invoice details in plaintext)
- **Collision risk** (two apps generate the same memo for different payments)

TEMPO-RECONCILE-MEMO-001 solves this by defining a strict, namespaced layout with forward compatibility through type code ranges.

---

## Spec

### Format

```
bytes32 memo (32 bytes total, big-endian):

Offset  Size   Field       Description
------  -----  ---------   ------------------------------------------
0       1      type        Payment type code (0x01-0x0F for v1)
1..8    8      issuerTag   Namespace tag (uint64 big-endian)
9..24   16     id16        ULID binary (16 bytes)
25..31  7      salt        Optional salt/metadata (default: zeros)
```

Total: 1 + 8 + 16 + 7 = 32 bytes.

### Field definitions

#### `type` byte (offset 0)

The first byte is the payment type code. There is no separate version nibble. Instead, forward compatibility is achieved through type code ranges:

- Type codes `0x01-0x0F` belong to v1 layout (this spec).
- Future layout versions claim higher ranges (`0x10-0x1F`, `0x20-0x2F`, etc.).
- A v1 decoder that sees `type >= 0x10` returns `null` -- it does not attempt to decode an unknown format.

**Type codes:**

| Code | Name | Description |
|------|------|-------------|
| 0x00 | (reserved) | Reserved, never valid |
| 0x01 | `invoice` | Invoice payment |
| 0x02 | `payroll` | Payroll/salary payment |
| 0x03 | `refund` | Refund for a previous payment |
| 0x04 | `batch` | Batch payout |
| 0x05 | `subscription` | Recurring subscription charge |
| 0x06..0x0E | (reserved) | Reserved for future v1 standard types |
| 0x0F | `custom` | Application-defined type |

#### `issuerTag` (offsets 1..8)

An 8-byte namespace identifier that prevents collisions between different applications using the same memo format.

**Derivation:**

```
issuerTag = first8bytes(keccak256(utf8(namespace)))
```

Where `namespace` is a human-readable string identifying the issuer. Examples:
- `"tempo-reconcile"` -> `0xfc7c8482914a04e8` (this SDK)
- `"my-app"` -> `0x3a180fb9d0177aa2`
- `"payroll-app"` -> `0x4c5cb70037f25f8c`

Stored as `uint64` big-endian.

Without a namespace, two independent apps could generate the same (type, ULID) pair for completely different payments. The issuerTag makes accidental collisions negligible. No central registry is required. Projects SHOULD document their namespace string for interoperability.

#### `id16` (offsets 9..24)

The primary identifier for the payment record. This is a **ULID** (Universally Unique Lexicographically Sortable Identifier) encoded as 16 binary bytes.

ULID fits exactly in 16 bytes. First 6 bytes are a millisecond timestamp, so IDs sort chronologically. String form is 26 Crockford base32 characters.

**Encoding:** Crockford base32 ULID string (26 chars) <-> 128-bit big-endian integer (16 bytes).

The ULID MUST be generated off-chain and stored in the application's database as the primary key for the expected payment record. The on-chain memo carries only the binary representation.

#### `salt` (offsets 25..31)

7 bytes of opaque, optional metadata. Default: all zeros.

Implementations SHOULD generate random salt when privacy is a concern. With zero salt, the combination of issuerTag + ULID timestamp (first 6 bytes) acts as a fingerprint by time and issuer. Random salt prevents this. Zero salt is acceptable when the memo is intended to be publicly matchable (e.g. donation receipts).

Parsers MUST treat this field as opaque bytes. Reconcilers MUST NOT require specific salt values for matching -- matching is always by `(issuerTag, id16)`.

#### Salt conventions (informational)

These are RECOMMENDED interpretations, not normative requirements. Applications MAY use salt for any purpose.

| Type | Convention | Layout |
|------|-----------|--------|
| invoice | Zeros or random | -- |
| payroll | Zeros or department/cost-center code | -- |
| refund | First 7 bytes of original payment's id16 | `salt[0..6]` = original id16 truncated |
| batch | Sequence index within the batch | `salt[0..1]` = uint16 BE, rest zeros |
| subscription | Billing period number | `salt[0..3]` = uint32 BE, rest zeros |
| custom | Application-defined | -- |

The refund convention creates a weak back-reference to the original payment (7 of 16 bytes). Combined with the issuerTag, this is enough for a database lookup. The batch sequence lets receivers verify completeness of a batch payout. The subscription period lets receivers distinguish January's payment from February's under the same ULID.

---

## Security

### No PII on-chain

The memo MUST NOT contain:
- Email addresses
- Phone numbers
- Real names
- Physical addresses
- Government IDs
- Credit card numbers
- Any data that identifies a natural person

The memo is a **reference**. The referenced data (invoice details, customer info) lives off-chain in the application's database.

### Collision resistance

The combination of `(issuerTag, id16)` provides sufficient uniqueness:
- `issuerTag`: 64-bit namespace (2^64 possible namespaces) -- birthday collision at ~4 billion issuers
- `id16`: 128-bit ULID (2^128 possible IDs per namespace)

Accidental collision probability is negligible for any practical use.

### Replay protection

The memo itself does not provide replay protection. Replay protection is handled by:
1. The blockchain's transaction uniqueness (each tx has unique hash)
2. The reconciler's deduplication by `(txHash, logIndex)`
3. The expected payment record being consumed after matching

---

## Encoding examples

### Example A: Invoice

```
Type:       invoice (0x01)
Namespace:  "tempo-reconcile"
ULID:       01MASW9NF6YW40J40H289H858P
Salt:       (zeros)

issuerTag = first8bytes(keccak256("tempo-reconcile")) = 0xfc7c8482914a04e8

type       = 01
issuerTag  = fc 7c 84 82 91 4a 04 e8
id16       = 01 a2 b3 c4 d5 e6 f7 08 09 10 11 12 13 14 15 16
salt       = 00 00 00 00 00 00 00

memo = 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000
```

### Example B: Batch with custom salt

```
Type:       batch (0x04)
Namespace:  "tempo-reconcile"
ULID:       01MASW9NF6YW40J40H289H858P
Salt:       ff010203040506

memo = 0x04fc7c8482914a04e801a2b3c4d5e6f7080910111213141516ff010203040506
```

---

## Decoding rules

1. Verify input is exactly 32 bytes (64 hex chars after `0x`).
2. Read byte 0 as `typeCode`.
3. If `typeCode` is not a known v1 code (`0x01-0x05`, `0x0F`), return `null`.
4. Read bytes 1..8 as `uint64` big-endian. This is `issuerTag`.
5. Read bytes 9..24 as 16-byte array. Convert to ULID string. This is `id16`.
6. Read bytes 25..31 as 7-byte array. This is `salt`.
7. Return the decoded `MemoV1` object with `v: 1` (implied by type code range).

**Critical: a decoder MUST gracefully handle memos that are not v1 format.** On-chain, there will be memos from other applications, other format versions, or raw bytes32 values. The decoder MUST return `null` for these, never throw.

---

## Usage (pseudo-code)

```
// Encode
tag       = first8bytes(keccak256("my-app"))
id16      = ulid_to_bytes("01MASW9NF6YW40J40H289H858P")
salt      = 0x00000000000000
memo      = concat(0x01, tag, id16, salt)            // 32 bytes
memo_hex  = "0x" + hex(memo)                         // pass to transferWithMemo

// Decode
if length(memo) != 32:       return null
if memo[0] < 0x01 or > 0x0F: return null
type      = type_name(memo[0])
tag       = uint64_be(memo[1..8])
id16      = memo[9..24]
ulid      = bytes_to_ulid(id16)
salt      = memo[25..31]
```

Reference implementations: [github.com/valpaq/tempo-reconciliation-sdk](https://github.com/valpaq/tempo-reconciliation-sdk) (TypeScript, Rust)

---

## Test vectors

See [vectors.json](../spec/vectors.json) for machine-readable test vectors that any encoder/decoder implementation must pass.

### Positive vectors

| Name | type | namespace | issuerTagHex | saltHex | memoRaw |
|------|------|-----------|--------------|---------|---------|
| invoice with default salt | invoice | tempo-reconcile | fc7c8482914a04e8 | 00000000000000 | `0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000` |
| payroll with default salt | payroll | payroll-app | 4c5cb70037f25f8c | 00000000000000 | `0x024c5cb70037f25f8c01a2b3c4d5e6f708091011121314151600000000000000` |
| refund with default salt | refund | my-app | 3a180fb9d0177aa2 | 00000000000000 | `0x033a180fb9d0177aa201a2b3c4d5e6f708091011121314151600000000000000` |
| batch with custom salt | batch | tempo-reconcile | fc7c8482914a04e8 | ff010203040506 | `0x04fc7c8482914a04e801a2b3c4d5e6f7080910111213141516ff010203040506` |
| custom type with full salt | custom | tempo-reconcile | fc7c8482914a04e8 | aabbccddeeff00 | `0x0ffc7c8482914a04e801a2b3c4d5e6f7080910111213141516aabbccddeeff00` |
| subscription with default salt | subscription | my-app | 3a180fb9d0177aa2 | 00000000000000 | `0x053a180fb9d0177aa201a2b3c4d5e6f708091011121314151600000000000000` |
| invoice with min ULID (all zeros) | invoice | my-app | 3a180fb9d0177aa2 | 00000000000000 | `0x013a180fb9d0177aa20000000000000000000000000000000000000000000000` |
| invoice with max ULID (all Zs) | invoice | tempo-reconcile | fc7c8482914a04e8 | 00000000000000 | `0x01fc7c8482914a04e8ffffffffffffffffffffffffffffffff00000000000000` |

### Negative vectors (decoder MUST return null)

| Name | reason |
|------|--------|
| all zeros | type 0x00 is reserved |
| type 0x00 | reserved |
| type 0x07 | reserved range (0x06-0x0E) |
| type 0x10 | future range, not a v1 type |
| too short (31 bytes) | not 32 bytes |
| too long (33 bytes) | not 32 bytes |
| type 0x06 (reserved) | reserved range (0x06-0x0E) |
| type 0x08 (reserved) | reserved range (0x06-0x0E) |
| type 0x09 (reserved) | reserved range (0x06-0x0E) |
| type 0x0E (reserved) | reserved range (0x06-0x0E) |

---

## Conformance

An implementation is **conformant** if it satisfies all of the following:

1. **Encoder** produces output that matches every positive test vector in `vectors.json` (exact byte-for-byte match).
2. **Decoder** returns `null` (or language equivalent) for every negative test vector.
3. **Encoder** and **decoder** follow every MUST rule in this document.
4. **Decoder** never throws/panics on arbitrary 32-byte input.

Partial implementations (decode-only or encode-only) are acceptable. A decode-only implementation MUST still pass requirements 2 and 4.

---

## Future

- New v1 standard types can fill codes `0x06-0x0E`. The format stays the same, but existing decoders will return `null` for unrecognized codes until updated. Adding a new type code is a breaking change for decoders (they start dropping data they could have processed). Bump the spec minor version when adding types.
- Future layout versions claim higher type code ranges: `0x10-0x1F`, `0x20-0x2F`, etc.
- v1 decoders that see `type >= 0x10` return `null` gracefully -- they never throw on unrecognized formats.
- No version byte. Versioning is implicit in the type code range.
- A voluntary namespace registry (mapping namespace string to issuerTag hex) may be useful if the standard sees adoption across multiple teams. No central registry is required by the spec, but an open list would help with issuer discovery.

---

## ISO 20022 mapping

Tempo positions its memo fields as ISO 20022-compatible. The following table maps MEMO-001 fields to their closest ISO 20022 Structured Remittance Information equivalents:

| MEMO-001 field | ISO 20022 path | Notes |
|----------------|----------------|-------|
| `type` | `RmtInf/Strd/RfrdDocInf/Tp/CdOrPrtry` | Payment purpose code |
| `issuerTag` | `RmtInf/Strd/CdtrRefInf/Ref` | Truncated to 8-byte hash |
| `id16` (ULID) | `RmtInf/Strd/RfrdDocInf/Nb` | Document reference number |
| `salt` | (no equivalent) | Privacy/uniqueness field |

This mapping is informational. MEMO-001 is not a subset of ISO 20022 -- it is a compact binary encoding optimized for 32-byte on-chain storage. The mapping shows conceptual equivalence for teams bridging between on-chain payments and traditional payment messaging.

---

## Comparison

| Standard | Chain | Memo size | On-chain data | Status |
|----------|-------|-----------|---------------|--------|
| TEMPO-RECONCILE-MEMO-001 (this) | Tempo | bytes32 fixed | Reference only | Stable |
| ERC-7699 | Ethereum | bytes (variable) | Flexible | Draft EIP |
| Stellar memo | Stellar | 28 bytes or 32-byte hash | Text or hash | Production |
| XRPL memos | XRP Ledger | Variable | Structured JSON | Production |
| Bitcoin OP_RETURN | Bitcoin | 80 bytes | Arbitrary | Production |

Our approach is closest to Stellar's memo field, adapted for the EVM/TIP-20 context.
