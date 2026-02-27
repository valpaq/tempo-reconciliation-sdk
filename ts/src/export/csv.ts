import type { MatchResult, MemoV1 } from "../types";
import { decodeMemoV1 } from "../memo/decode";

function escapeCsv(value: string): string {
  if (value.includes(",") || value.includes('"') || value.includes("\n") || value.includes("\r")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function formatAmount(amount: bigint, decimals = 6): string {
  const str = amount.toString();
  if (str.length <= decimals) {
    return `0.${str.padStart(decimals, "0")}`;
  }
  return `${str.slice(0, str.length - decimals)}.${str.slice(str.length - decimals)}`;
}

/**
 * Serialize reconciliation results as CSV.
 *
 * Fixed columns: timestamp, block_number, tx_hash, log_index, chain_id, from, to,
 * token, amount_raw, amount_human, memo_raw, memo_type, memo_ulid, memo_issuer_tag,
 * status, expected_amount, expected_from, expected_to, expected_due_at,
 * reason, overpaid_by, is_late, remaining_amount. Dynamic `meta_*` columns are
 * appended for any keys found in `expected.meta` across all results.
 *
 * @param results - Array of MatchResult from Reconciler
 * @returns CSV string with header row, newline-terminated
 * @example
 * ```ts
 * const report = reconciler.report();
 * const csv = exportCsv([...report.matched, ...report.issues]);
 * fs.writeFileSync("reconciliation.csv", csv);
 * ```
 */
export function exportCsv(results: MatchResult[]): string {
  const metaKeys = new Set<string>();
  for (const r of results) {
    if (r.expected?.meta) {
      for (const key of Object.keys(r.expected.meta)) {
        metaKeys.add(key);
      }
    }
  }
  const sortedMetaKeys = [...metaKeys].sort();

  const columns = [
    "timestamp",
    "block_number",
    "tx_hash",
    "log_index",
    "chain_id",
    "from",
    "to",
    "token",
    "amount_raw",
    "amount_human",
    "memo_raw",
    "memo_type",
    "memo_ulid",
    "memo_issuer_tag",
    "status",
    "expected_amount",
    "expected_from",
    "expected_to",
    "expected_due_at",
    "reason",
    "overpaid_by",
    "is_late",
    "remaining_amount",
    ...sortedMetaKeys.map((k) => `meta_${k}`),
  ];

  const rows = [columns.map(escapeCsv).join(",")];

  for (const r of results) {
    const existing = r.payment.memo;
    const memo: MemoV1 | null =
      existing && typeof existing === "object" && "v" in existing
        ? existing
        : r.payment.memoRaw
          ? decodeMemoV1(r.payment.memoRaw)
          : null;
    const values = [
      r.payment.timestamp?.toString() ?? "",
      r.payment.blockNumber.toString(),
      r.payment.txHash,
      r.payment.logIndex.toString(),
      r.payment.chainId.toString(),
      r.payment.from,
      r.payment.to,
      r.payment.token,
      r.payment.amount.toString(),
      formatAmount(r.payment.amount),
      r.payment.memoRaw ?? "",
      memo?.t ?? "",
      memo?.ulid ?? "",
      memo?.issuerTag?.toString() ?? "",
      r.status,
      r.expected?.amount.toString() ?? "",
      r.expected?.from ?? "",
      r.expected?.to ?? "",
      r.expected?.dueAt?.toString() ?? "",
      r.reason ?? "",
      r.overpaidBy?.toString() ?? "",
      r.isLate ? "true" : "",
      r.remainingAmount?.toString() ?? "",
      ...sortedMetaKeys.map((k) => r.expected?.meta?.[k] ?? ""),
    ];
    rows.push(values.map(escapeCsv).join(","));
  }

  return rows.join("\n") + "\n";
}
