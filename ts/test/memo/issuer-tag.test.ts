import { describe, it, expect } from "vitest";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";

describe("issuerTagFromNamespace", () => {
  it("returns known value for tempo-reconcile", () => {
    expect(issuerTagFromNamespace("tempo-reconcile")).toBe(0xfc7c8482914a04e8n);
  });

  it("returns known value for my-app", () => {
    expect(issuerTagFromNamespace("my-app")).toBe(0x3a180fb9d0177aa2n);
  });

  it("returns known value for payroll-app", () => {
    expect(issuerTagFromNamespace("payroll-app")).toBe(0x4c5cb70037f25f8cn);
  });

  it("is deterministic", () => {
    const a = issuerTagFromNamespace("some-namespace");
    const b = issuerTagFromNamespace("some-namespace");
    expect(a).toBe(b);
  });

  it("different namespaces produce different tags", () => {
    const a = issuerTagFromNamespace("app-1");
    const b = issuerTagFromNamespace("app-2");
    expect(a).not.toBe(b);
  });

  it("handles empty string", () => {
    const tag = issuerTagFromNamespace("");
    expect(typeof tag).toBe("bigint");
  });

  it("handles unicode", () => {
    const tag = issuerTagFromNamespace("приложение");
    expect(typeof tag).toBe("bigint");
  });
});
