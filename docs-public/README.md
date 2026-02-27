# tempo-reconcile docs

Documentation for `@tempo-reconcile/sdk`.

## Guides

- [Getting started](./GETTING-STARTED.md) -- install, encode a memo, watch payments, reconcile, export
- [Examples](./EXAMPLES.md) -- invoice reconciliation, batch payouts, webhook integration, memo decoding

## Reference

- [API](./API.md) -- every function, type, and option
- [Memo spec](../spec/MEMO-SPEC.md) -- TEMPO-RECONCILE-MEMO-001 bytes32 layout

## Quick reference

```bash
npm i @tempo-reconcile/sdk
```

```typescript
import { encodeMemoV1, decodeMemoV1, issuerTagFromNamespace } from '@tempo-reconcile/sdk'
import { watchTip20Transfers, Reconciler, exportCsv } from '@tempo-reconcile/sdk'
```

Moderato testnet: chain ID `42431`, RPC `https://rpc.moderato.tempo.xyz`

pathUSD: `0x20C0000000000000000000000000000000000000` (6 decimals)
