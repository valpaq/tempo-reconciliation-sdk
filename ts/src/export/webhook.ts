import { createHmac } from "node:crypto";
import { keccak_256 } from "@noble/hashes/sha3.js";
import type { MatchResult, WebhookBatchError, WebhookOptions, WebhookResult } from "../types";

export function sign(payload: string, secret: string): string {
  return createHmac("sha256", secret).update(payload).digest("hex");
}

/**
 * POST reconciliation results to a webhook endpoint in batches.
 *
 * Each batch is sent with an idempotency key and optional HMAC-SHA256 signature.
 * Retries on 5xx / 429 / 408 / network errors with exponential backoff (1s, 2s, 4s...).
 * Other 4xx errors are not retried.
 *
 * @param options - Webhook URL, results, secret, batch size, and retry config
 * @returns Counts of sent/failed events and per-batch error details
 * @example
 * ```ts
 * const { sent, failed, errors } = await sendWebhook({
 *   url: "https://api.example.com/webhooks/payments",
 *   results: report.matched,
 *   secret: "whsec_...",
 * });
 * ```
 */
export async function sendWebhook(options: WebhookOptions): Promise<WebhookResult> {
  const {
    url,
    results,
    secret,
    batchSize = 50,
    maxRetries = 3,
    timeoutMs = 30_000,
    onBatchError,
  } = options;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`Invalid webhook URL: ${url}`);
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error(`Invalid webhook URL protocol: ${parsed.protocol}`);
  }

  const fetchFn = options.fetch ?? globalThis.fetch;
  let sent = 0;
  let failed = 0;
  const errors: WebhookBatchError[] = [];

  for (let i = 0; i < results.length; i += batchSize) {
    const batch = results.slice(i, i + batchSize);
    const outcome = await sendBatch(url, batch, secret, maxRetries, timeoutMs, fetchFn);
    if (outcome.ok) {
      sent += batch.length;
    } else {
      failed += batch.length;
      const batchError: WebhookBatchError = {
        results: batch,
        statusCode: outcome.statusCode,
        error: outcome.error,
      };
      errors.push(batchError);
      onBatchError?.(batchError);
    }
  }

  return { sent, failed, errors };
}

type BatchOutcome = { ok: true } | { ok: false; statusCode?: number; error?: string };

async function sendBatch(
  url: string,
  events: MatchResult[],
  secret: string | undefined,
  maxRetries: number,
  timeoutMs: number,
  fetchFn: typeof globalThis.fetch,
): Promise<BatchOutcome> {
  const timestamp = Math.floor(Date.now() / 1000);
  // Stable key derived from batch content — same across retries and process restarts.
  const batchFingerprint = events
    .map((e) => `${e.payment.txHash}:${e.payment.logIndex}`)
    .join("|");
  const idempotencyKey = Array.from(
    keccak_256(new TextEncoder().encode(batchFingerprint)),
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");

  const body = JSON.stringify({
    id: `whevt_${idempotencyKey}`,
    timestamp,
    events: events.map((e) => ({
      status: e.status,
      payment: {
        chainId: e.payment.chainId,
        txHash: e.payment.txHash,
        logIndex: e.payment.logIndex,
        amount: e.payment.amount.toString(),
        from: e.payment.from,
        to: e.payment.to,
        token: e.payment.token,
        blockNumber: e.payment.blockNumber.toString(),
        memoRaw: e.payment.memoRaw,
      },
      expected: e.expected
        ? {
            amount: e.expected.amount.toString(),
            meta: e.expected.meta,
          }
        : undefined,
      reason: e.reason,
    })),
  });

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "X-Tempo-Reconcile-Idempotency-Key": idempotencyKey,
    "X-Tempo-Reconcile-Timestamp": timestamp.toString(),
  };

  if (secret) {
    headers["X-Tempo-Reconcile-Signature"] = sign(body, secret);
  }

  let lastStatusCode: number | undefined;
  let lastError: string | undefined;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      const res = await fetchFn(url, {
        method: "POST",
        headers,
        body,
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (res.ok) return { ok: true };
      lastStatusCode = res.status;
      lastError = `HTTP ${res.status}`;
      if (res.status >= 400 && res.status < 500 && res.status !== 429 && res.status !== 408) {
        return { ok: false, statusCode: res.status, error: lastError };
      }
    } catch (err) {
      lastStatusCode = undefined;
      lastError = err instanceof Error ? err.message : "network error";
    }

    if (attempt < maxRetries) {
      const delay = Math.min(1000 * 2 ** attempt, 30_000);
      await new Promise((r) => setTimeout(r, delay));
    }
  }

  return { ok: false, statusCode: lastStatusCode, error: lastError };
}
