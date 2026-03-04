# tempo-reconcile CLI

Command-line tool for TIP-20 payment reconciliation on Tempo. Watch for on-chain transfers, decode structured memos, match payments to expected records, and export results — no code required.

## Install

```bash
cargo install tempo-reconcile
```

Or download a pre-built binary from the [releases page](https://github.com/valpaq/tempo-reconciliation-sdk/releases).

## Commands

| Command | Description |
|---------|-------------|
| `watch` | Stream TransferWithMemo events to stdout or a file |
| `run` | Reconcile a JSONL events file against a CSV of expected payments |
| `memo encode` | Encode a memo from type, namespace, and ULID |
| `memo decode` | Decode a bytes32 memo |
| `memo generate` | Generate a new ULID and encode a memo |
| `memo issuer-tag` | Compute the issuer tag for a namespace string |

Use `tempo-reconcile --help` or `tempo-reconcile <command> --help` for full option lists.

## Workflow: file-based (no database)

```bash
# 1. Watch for incoming payments, write events to a file
tempo-reconcile watch \
  --rpc https://rpc.moderato.tempo.xyz \
  --chain-id 42431 \
  --token 0x20C0000000000000000000000000000000000000 \
  --to 0xYourAddress \
  --out events.jsonl

# 2. Reconcile against your expected payments CSV
tempo-reconcile run \
  --events events.jsonl \
  --expected invoices.csv \
  --out report.csv

# 3. Open report.csv in a spreadsheet
```

## Workflow: memo utilities

```bash
# Encode a memo
tempo-reconcile memo encode \
  --type invoice \
  --namespace my-company \
  --ulid 01MASW9NF6YW40J40H289H858P

# Decode a memo
tempo-reconcile memo decode 0x01fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000

# Generate a fresh memo (new ULID)
tempo-reconcile memo generate --type invoice --namespace my-company

# Compute issuer tag for a namespace
tempo-reconcile memo issuer-tag my-company
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (RPC failure, parse error, reconciler error) |
| 2 | Invalid arguments (clap validation) |
| 5 | File I/O error (events file, expected CSV, output file) |

## Environment variables

| Variable | Description |
|----------|-------------|
| `TEMPO_RPC_URL` | Default RPC URL |
| `TEMPO_CHAIN_ID` | Default chain ID |
| `TEMPO_TOKEN` | Default token address |
| `TEMPO_RECONCILE_NAMESPACE` | Default namespace for memo commands |

## Full reference

See [docs/CLI.md](../../docs/CLI.md) for complete command documentation including all options, output formats, and workflow examples.
