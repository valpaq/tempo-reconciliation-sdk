import type { MatchResult } from "../types";

function replacer(_key: string, value: unknown): unknown {
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (value instanceof Uint8Array) {
    return Array.from(value);
  }
  return value;
}

/**
 * Serialize reconciliation results as a pretty-printed JSON array.
 *
 * `bigint` values are serialized as decimal strings; `Uint8Array` as number arrays.
 *
 * @param results - Array of MatchResult from Reconciler
 * @returns Pretty-printed JSON string (2-space indent)
 * @example
 * ```ts
 * const json = exportJson(report.matched);
 * fs.writeFileSync("matched.json", json);
 * ```
 */
export function exportJson(results: MatchResult[]): string {
  return JSON.stringify(results, replacer, 2);
}

/**
 * Serialize reconciliation results as newline-delimited JSON (one object per line).
 *
 * Suitable for streaming ingestion into log systems and databases.
 *
 * @param results - Array of MatchResult from Reconciler
 * @returns JSONL string (one JSON object per line), newline-terminated
 * @example
 * ```ts
 * const jsonl = exportJsonl(report.matched);
 * fs.appendFileSync("events.jsonl", jsonl);
 * ```
 */
export function exportJsonl(results: MatchResult[]): string {
  if (results.length === 0) return "";
  return results.map((r) => JSON.stringify(r, replacer)).join("\n") + "\n";
}
