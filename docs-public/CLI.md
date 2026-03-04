# CLI reference

`tempo-reconcile` is a command-line tool for TIP-20 payment reconciliation on Tempo.
No code required — watch for on-chain transfers, decode memos, reconcile against expected payments, and export results.

## Install

```bash
cargo install tempo-reconcile
```

Or download a pre-built binary from the [releases page](https://github.com/valpaq/tempo-reconciliation-sdk/releases).

## Global flags

| Flag | Description |
|------|-------------|
| `--json` | Machine-readable JSON output instead of human-readable text |
| `--version` | Print version and exit |
| `--help` | Print help and exit |

## Commands

```
tempo-reconcile <COMMAND>

Commands:
  watch    Stream TransferWithMemo events to stdout or a file
  run      Reconcile a JSONL events file against a CSV of expected payments
  memo     Encode, decode, generate, and inspect memo values
```

---

## `watch`

Stream incoming `TransferWithMemo` events from the chain. Runs until `Ctrl+C`.

```bash
tempo-reconcile watch \
  --rpc https://rpc.moderato.tempo.xyz \
  --chain-id 42431 \
  --token 0x20C0000000000000000000000000000000000000 \
  --to 0xYourAddress \
  --out events.jsonl
```

### Options

| Flag | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `--rpc` | string | yes | `$TEMPO_RPC_URL` | Tempo RPC URL |
| `--chain-id` | u64 | yes | `$TEMPO_CHAIN_ID` | Chain ID (Moderato: `42431`) |
| `--token` | address | yes | `$TEMPO_TOKEN` | TIP-20 token contract address |
| `--to` | address | no | — | Filter by recipient address |
| `--from` | address | no | — | Filter by sender address |
| `--start-block` | u64 | no | latest | Block to start from |
| `--batch-size` | u64 | no | `2000` | Blocks per `eth_getLogs` call |
| `--out` | path | no | stdout | JSONL output file (appended) |
| `--include-transfer-only` | flag | no | off | Include plain `Transfer` events without a memo |
| `--poll-interval` | ms | no | `1000` | Polling interval in milliseconds |
| `--rpc-timeout` | ms | no | `30000` | Per-request RPC timeout in milliseconds |

### Environment variables

| Variable | Overrides |
|----------|-----------|
| `TEMPO_RPC_URL` | `--rpc` |
| `TEMPO_CHAIN_ID` | `--chain-id` |
| `TEMPO_TOKEN` | `--token` |

### Output format

Each output line is a JSON object:

```json
{
  "chainId": 42431,
  "blockNumber": 1234,
  "txHash": "0xabc...",
  "logIndex": 0,
  "token": "0x20c0...",
  "from": "0xsender",
  "to": "0xrecipient",
  "amount": "10000000",
  "memoRaw": "0x01fc7c...",
  "timestamp": 1709123456
}
```

To decode `memoRaw` into type and ULID, pipe through `memo decode`:

```bash
tempo-reconcile memo decode 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000
```

### Behavior

- Appends to `--out` — safe to stop and restart
- Deduplicates by `txHash + logIndex` within a session
- Prints a summary to stderr on exit

---

## `run`

Reconcile a JSONL events file against a CSV of expected payments. Offline — no network, no database.

```bash
tempo-reconcile run \
  --events events.jsonl \
  --expected invoices.csv \
  --out report.csv
```

### Options

| Flag | Type | Required | Default | Description |
|------|------|----------|---------|-------------|
| `--events` | path | yes | — | JSONL file produced by `watch` |
| `--expected` | path | yes | — | CSV of expected payments |
| `--out` | path | no | stdout | Output report file |
| `--format` | string | no | `csv` | Output format: `csv`, `json`, `jsonl` |
| `--issuer-namespace` | string | no | `$TEMPO_RECONCILE_NAMESPACE` | Only match memos with this namespace's issuer tag |
| `--strict-sender` | flag | no | off | Require `from` address to match expected |
| `--tolerance` | u32 | no | `0` | Amount tolerance in basis points (max `10000` = 100%) |
| `--allow-partial` | flag | no | off | Accumulate partial payments toward expected total |
| `--reject-expired` | flag | no | off | Reject payments received after `due_at` |
| `--strict-amount` | flag | no | off | Reject overpayments (default: accept overpayments) |
| `--partial-tolerance-mode` | string | no | `final` | How tolerance applies to partials: `final` or `each` |

### Expected payments CSV format

Required columns: `memo_raw`, `token`, `to`, `amount`

Optional columns: `from`, `due_at` (Unix timestamp), any `meta.*` column

```csv
memo_raw,token,to,amount,from,due_at,meta.invoiceId
0x01fc7c...,0x20c0...,0xrecipient,10000000,,"INV-001"
0x02fc7c...,0x20c0...,0xrecipient,25000000,0xsender,1709200000,"INV-002"
```

### Match statuses

| Status | Meaning |
|--------|---------|
| `matched` | Memo found, amount correct, all checks passed |
| `mismatch_amount` | Memo found, amount outside tolerance |
| `mismatch_token` | Memo found, wrong token contract |
| `mismatch_party` | Memo found, wrong `to` or `from` (when `--strict-sender`) |
| `unknown_memo` | Memo present on-chain but not in expected list |
| `no_memo` | Transfer without memo |
| `expired` | Payment arrived after `due_at` (only when `--reject-expired`) |
| `partial` | Partial payment accumulated, not yet complete |

### Summary output (stderr)

```
Reconciliation Report
=====================
Total expected:   10
Total received:   12
Matched:          8
Issues:           4
  unknown_memo:     2
  mismatch_amount:  1
  no_memo:          1
Pending:          2
```

With `--json` global flag the summary is JSON:

```json
{
  "matchedCount": 8,
  "issueCount": 4,
  "pendingCount": 2,
  "totalExpected": 10,
  "totalReceived": 12,
  "unknownMemoCount": 2,
  "noMemoCount": 1,
  "mismatchAmountCount": 1
}
```

### `--partial-tolerance-mode`

| Mode | Behavior |
|------|----------|
| `final` (default) | Tolerance applied to the cumulative total. Partial payments can individually fall below tolerance as long as the sum is close enough. |
| `each` | Each individual partial payment must pass the tolerance check on its own. The final cumulative total must reach the full expected amount. |

---

## `memo`

Utility subcommands for encoding and inspecting memo values.

### `memo encode`

Encode a memo from its parts.

```bash
tempo-reconcile memo encode \
  --type invoice \
  --namespace my-company \
  --ulid 01MASW9NF6YW40J40H289H858P
# 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000
```

| Flag | Required | Description |
|------|----------|-------------|
| `--type` | yes | `invoice`, `payroll`, `refund`, `batch`, `subscription`, `custom` |
| `--namespace` | yes | Application namespace (e.g. `my-company`) |
| `--ulid` | yes | 26-char Crockford base32 ULID |
| `--salt` | no | 7-byte salt as 14 hex chars. Default: all zeros |

With `--json`: `{"memoRaw":"0x01fc7c..."}`

### `memo decode`

Decode a bytes32 memo.

```bash
tempo-reconcile memo decode 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000
# Type:       invoice (0x01)
# IssuerTag:  0xfc7c8482914a04e8
# ULID:       01MASW9NF6YW40J40H289H858P
# Salt:       00000000000000
```

Exits with code 1 if the memo is not valid v1 format.

With `--json`: `{"type":"invoice","issuerTag":"0xfc7c8482914a04e8","ulid":"...","salt":"..."}`

### `memo generate`

Generate a fresh memo with a new ULID.

```bash
tempo-reconcile memo generate --type invoice --namespace my-company
# ULID:  01NEWULID4XVQZRBKP2T7FCPWV
# Memo:  0x01fc7c...
```

| Flag | Required | Description |
|------|----------|-------------|
| `--type` | yes | Memo type |
| `--namespace` | yes | Application namespace |
| `--random-salt` | no | Use random 7-byte salt instead of zeros |

With `--json`: `{"ulid":"...","memoRaw":"0x...","type":"invoice","issuerTag":"0x..."}`

### `memo issuer-tag`

Compute the issuer tag for a namespace string.

```bash
tempo-reconcile memo issuer-tag tempo-reconcile
# 0xfc7c8482914a04e8 (18193562290988123368)
```

With `--json`: `{"namespace":"tempo-reconcile","issuerTag":"0xfc7c8482914a04e8","issuerTagDecimal":18193562290988123368}`

---

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (invalid memo, reconciler error, bad argument value) |
| 2 | Invalid arguments (missing required flag, unknown option) |
| 5 | File I/O error (file not found, permission denied, write failure) |

---

## Workflow: file-based reconciliation

The primary workflow — no database required.

```bash
# 1. Collect events while payments arrive (stop with Ctrl+C)
tempo-reconcile watch \
  --rpc https://rpc.moderato.tempo.xyz \
  --chain-id 42431 \
  --token 0x20C0000000000000000000000000000000000000 \
  --to 0xYourAddress \
  --out events.jsonl

# 2. Prepare your expected payments CSV (from your billing system)
# memo_raw,token,to,amount,meta.invoiceId
# 0x01fc7c...,0x20c0...,0xYourAddress,10000000,INV-001

# 3. Reconcile
tempo-reconcile run \
  --events events.jsonl \
  --expected invoices.csv \
  --out report.csv

# 4. Open report.csv in your spreadsheet or import into your ERP
```

## Workflow: generate a memo and share it

```bash
# Generate a new payment reference
tempo-reconcile memo generate --type invoice --namespace acme-billing

# Share the Memo line with your payer:
# "Please call transferWithMemo(recipient, amount, 0x01fc7c...)"
```

## Workflow: inspect an on-chain memo

```bash
tempo-reconcile memo decode 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000
```
