import { createHmac } from "node:crypto";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { sendWebhook, sign } from "../../src/export/webhook";
import type { MatchResult } from "../../src/types";
import { encodeMemoV1 } from "../../src/memo/encode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";

const TAG = issuerTagFromNamespace("test-app");
const MEMO = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: "01MASW9NF6YW40J40H289H858P" });

function makeResult(overrides: Partial<MatchResult> = {}): MatchResult {
  return {
    status: "matched",
    payment: {
      chainId: 42431,
      blockNumber: 100n,
      txHash: "0xaaaa000000000000000000000000000000000000000000000000000000000001",
      logIndex: 0,
      token: "0x20C0000000000000000000000000000000000000",
      from: "0x2222222222222222222222222222222222222222",
      to: "0x1111111111111111111111111111111111111111",
      amount: 10_000_000n,
      memoRaw: MEMO,
    },
    expected: {
      memoRaw: MEMO,
      token: "0x20C0000000000000000000000000000000000000",
      to: "0x1111111111111111111111111111111111111111",
      amount: 10_000_000n,
    },
    ...overrides,
  };
}

describe("sendWebhook", () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let fetchSpy: any;

  beforeEach(() => {
    fetchSpy = vi.spyOn(globalThis, "fetch");
    fetchSpy.mockResolvedValue(new Response("ok", { status: 200 }));
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("sends POST with correct JSON body structure", async () => {
    await sendWebhook({ url: "https://hook.test/cb", results: [makeResult()] });

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0]!;
    expect(url).toBe("https://hook.test/cb");
    expect(init?.method).toBe("POST");

    const body = JSON.parse(init?.body as string);
    expect(body.id).toMatch(/^whevt_/);
    expect(typeof body.timestamp).toBe("number");
    expect(body.events).toHaveLength(1);
    expect(body.events[0].status).toBe("matched");

    const payment = body.events[0].payment;
    expect(payment.chainId).toBe(42431);
    expect(payment.logIndex).toBe(0);
    expect(payment.memoRaw).toBe(MEMO);
    expect(payment.txHash).toMatch(/^0x/);
    expect(payment.token).toMatch(/^0x/);
  });

  it("includes HMAC signature header when secret is set", async () => {
    await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      secret: "test-secret-key",
    });

    const [, init] = fetchSpy.mock.calls[0]!;
    const headers = init?.headers as Record<string, string>;
    expect(headers["X-Tempo-Reconcile-Signature"]).toBeDefined();
    expect(headers["X-Tempo-Reconcile-Signature"]).toMatch(/^[a-f0-9]{64}$/);
  });

  it("produces correct HMAC-SHA256 of the request body", async () => {
    const secret = "verify-me";
    await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      secret,
    });

    const [, init] = fetchSpy.mock.calls[0]!;
    const body = init?.body as string;
    const headers = init?.headers as Record<string, string>;
    const expected = createHmac("sha256", secret).update(body).digest("hex");
    expect(headers["X-Tempo-Reconcile-Signature"]).toBe(expected);
  });

  it("omits signature header when no secret", async () => {
    await sendWebhook({ url: "https://hook.test/cb", results: [makeResult()] });

    const [, init] = fetchSpy.mock.calls[0]!;
    const headers = init?.headers as Record<string, string>;
    expect(headers["X-Tempo-Reconcile-Signature"]).toBeUndefined();
  });

  it("includes idempotency key and timestamp headers", async () => {
    await sendWebhook({ url: "https://hook.test/cb", results: [makeResult()] });

    const [, init] = fetchSpy.mock.calls[0]!;
    const headers = init?.headers as Record<string, string>;
    expect(headers["X-Tempo-Reconcile-Idempotency-Key"]).toBeDefined();
    expect(headers["X-Tempo-Reconcile-Timestamp"]).toBeDefined();
  });

  it("batches results according to batchSize", async () => {
    const results = Array.from({ length: 5 }, () => makeResult());
    await sendWebhook({ url: "https://hook.test/cb", results, batchSize: 2 });

    // 5 results / batchSize 2 = 3 batches
    expect(fetchSpy).toHaveBeenCalledTimes(3);
  });

  it("returns correct sent/failed counts on success", async () => {
    const results = [makeResult(), makeResult()];
    const out = await sendWebhook({ url: "https://hook.test/cb", results });
    expect(out.sent).toBe(2);
    expect(out.failed).toBe(0);
    expect(out.errors).toHaveLength(0);
  });

  it("retries on 429 Too Many Requests", async () => {
    fetchSpy
      .mockResolvedValueOnce(new Response("rate limit", { status: 429 }))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });

    expect(fetchSpy).toHaveBeenCalledTimes(2);
    expect(out.sent).toBe(1);
  });

  it("retries on 408 Request Timeout", async () => {
    fetchSpy
      .mockResolvedValueOnce(new Response("timeout", { status: 408 }))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });

    expect(fetchSpy).toHaveBeenCalledTimes(2);
    expect(out.sent).toBe(1);
  });

  it("retries on 500 errors", async () => {
    fetchSpy
      .mockResolvedValueOnce(new Response("err", { status: 500 }))
      .mockResolvedValueOnce(new Response("ok", { status: 200 }));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });

    expect(fetchSpy).toHaveBeenCalledTimes(2);
    expect(out.sent).toBe(1);
    expect(out.failed).toBe(0);
  });

  it("does not retry on 400 errors", async () => {
    fetchSpy.mockResolvedValue(new Response("bad", { status: 400 }));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });

    expect(fetchSpy).toHaveBeenCalledOnce();
    expect(out.sent).toBe(0);
    expect(out.failed).toBe(1);
  });

  it("handles network errors with retries", async () => {
    fetchSpy.mockRejectedValue(new Error("network down"));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 1,
    });

    // initial attempt + 1 retry = 2 calls
    expect(fetchSpy).toHaveBeenCalledTimes(2);
    expect(out.sent).toBe(0);
    expect(out.failed).toBe(1);
  });

  it("serializes bigint as string in body", async () => {
    await sendWebhook({ url: "https://hook.test/cb", results: [makeResult()] });

    const [, init] = fetchSpy.mock.calls[0]!;
    const body = JSON.parse(init?.body as string);
    expect(body.events[0].payment.amount).toBe("10000000");
    expect(body.events[0].payment.blockNumber).toBe("100");
  });

  it("includes overpaidBy, remainingAmount, and isLate in body", async () => {
    const result = makeResult({
      overpaidBy: 500_000n,
      remainingAmount: 3_000_000n,
      isLate: true,
    });
    await sendWebhook({ url: "https://hook.test/cb", results: [result] });

    const [, init] = fetchSpy.mock.calls[0]!;
    const body = JSON.parse(init?.body as string);
    const event = body.events[0];
    expect(event.overpaidBy).toBe("500000");
    expect(event.remainingAmount).toBe("3000000");
    expect(event.isLate).toBe(true);
  });

  it("omits overpaidBy and remainingAmount when undefined", async () => {
    await sendWebhook({ url: "https://hook.test/cb", results: [makeResult()] });

    const [, init] = fetchSpy.mock.calls[0]!;
    const body = JSON.parse(init?.body as string);
    const event = body.events[0];
    expect(event.overpaidBy).toBeUndefined();
    expect(event.remainingAmount).toBeUndefined();
    expect(event.isLate).toBeUndefined();
  });

  it("populates errors array with failed batch details", async () => {
    fetchSpy.mockResolvedValue(new Response("err", { status: 500 }));

    const results = [makeResult(), makeResult(), makeResult()];
    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results,
      batchSize: 2,
      maxRetries: 0,
    });

    expect(out.failed).toBe(3);
    expect(out.errors).toHaveLength(2);
    expect(out.errors[0]!.results).toHaveLength(2);
    expect(out.errors[0]!.statusCode).toBe(500);
    expect(out.errors[0]!.error).toBe("HTTP 500");
    expect(out.errors[1]!.results).toHaveLength(1);
  });

  it("calls onBatchError for each failed batch", async () => {
    fetchSpy.mockResolvedValue(new Response("err", { status: 500 }));
    const errors: unknown[] = [];

    await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult(), makeResult()],
      batchSize: 1,
      maxRetries: 0,
      onBatchError: (err) => errors.push(err),
    });

    expect(errors).toHaveLength(2);
  });

  it("returns errors with network error message", async () => {
    fetchSpy.mockRejectedValue(new Error("ECONNREFUSED"));

    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 0,
    });

    expect(out.errors).toHaveLength(1);
    expect(out.errors[0]!.error).toBe("ECONNREFUSED");
    expect(out.errors[0]!.statusCode).toBeUndefined();
  });

  it("throws on invalid URL", async () => {
    await expect(sendWebhook({ url: "not-a-url", results: [makeResult()] })).rejects.toThrow(
      "Invalid webhook URL: not-a-url",
    );
  });

  it("throws on non-http/https URL protocol", async () => {
    await expect(
      sendWebhook({ url: "ftp://hook.test/cb", results: [makeResult()] }),
    ).rejects.toThrow("Invalid webhook URL protocol: ftp:");
  });

  it("returns zero counts for empty results array", async () => {
    const out = await sendWebhook({ url: "https://hook.test/cb", results: [] });
    expect(out.sent).toBe(0);
    expect(out.failed).toBe(0);
    expect(out.errors).toHaveLength(0);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("idempotency key is stable across calls with same results", async () => {
    const results = [makeResult()];
    await sendWebhook({ url: "https://hook.test/cb", results });
    await sendWebhook({ url: "https://hook.test/cb", results });

    const body1 = JSON.parse(fetchSpy.mock.calls[0]![1]?.body as string);
    const body2 = JSON.parse(fetchSpy.mock.calls[1]![1]?.body as string);
    expect(body1.id).toBe(body2.id);
  });

  it("does not retry on 401 Unauthorized", async () => {
    fetchSpy.mockResolvedValue(new Response("Unauthorized", { status: 401 }));
    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });
    expect(fetchSpy).toHaveBeenCalledOnce();
    expect(out.failed).toBe(1);
  });

  it("does not retry on 403 Forbidden", async () => {
    fetchSpy.mockResolvedValue(new Response("Forbidden", { status: 403 }));
    const out = await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      maxRetries: 3,
    });
    expect(fetchSpy).toHaveBeenCalledOnce();
    expect(out.failed).toBe(1);
  });

  it("passes AbortSignal.timeout to fetch", async () => {
    const customFetch = vi.fn().mockResolvedValue(new Response("ok", { status: 200 }));

    await sendWebhook({
      url: "https://hook.test/cb",
      results: [makeResult()],
      timeoutMs: 5000,
      fetch: customFetch,
    });

    expect(customFetch).toHaveBeenCalledOnce();
    const [, init] = customFetch.mock.calls[0]!;
    expect(init?.signal).toBeInstanceOf(AbortSignal);
  });
});

describe("sign", () => {
  it("returns hex HMAC-SHA256", async () => {
    const result = await sign("payload", "secret");
    const expected = createHmac("sha256", "secret").update("payload").digest("hex");
    expect(result).toBe(expected);
  });

  it("returns different signatures for different payloads", async () => {
    expect(await sign("a", "secret")).not.toBe(await sign("b", "secret"));
  });

  it("returns different signatures for different secrets", async () => {
    expect(await sign("payload", "s1")).not.toBe(await sign("payload", "s2"));
  });
});
