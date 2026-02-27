import { describe, it, expect } from "vitest";
import { encodeMemoV1, randomSalt } from "../../src/memo/encode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";

describe("encodeMemoV1", () => {
  const TAG = issuerTagFromNamespace("test-app");

  it("produces 66-char hex string (0x + 64 hex chars)", () => {
    const memo = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
    });
    expect(memo).toMatch(/^0x[0-9a-f]{64}$/);
  });

  it("sets type byte to the type code", () => {
    const memo = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
    });
    const firstByte = parseInt(memo.slice(2, 4), 16);
    expect(firstByte).toBe(0x01); // invoice type code
  });

  it("encodes each type correctly", () => {
    const types = [
      ["invoice", 0x1],
      ["payroll", 0x2],
      ["refund", 0x3],
      ["batch", 0x4],
      ["subscription", 0x5],
      ["custom", 0xf],
    ] as const;

    for (const [type, code] of types) {
      const memo = encodeMemoV1({
        type,
        issuerTag: TAG,
        ulid: "01MASW9NF6YW40J40H289H858P",
      });
      const firstByte = parseInt(memo.slice(2, 4), 16);
      expect(firstByte).toBe(code);
    }
  });

  it("defaults salt to zeros", () => {
    const memo = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
    });
    // bytes 25-31 = salt, at hex offset 50 (25 * 2), 7 bytes = 14 hex chars
    const saltHex = memo.slice(2 + 50);
    expect(saltHex).toBe("00000000000000");
  });

  it("includes custom salt", () => {
    const salt = new Uint8Array([0xff, 1, 2, 3, 4, 5, 6]);
    const memo = encodeMemoV1({
      type: "batch",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
      salt,
    });
    // salt at hex offset 50, 7 bytes = 14 hex chars
    const saltHex = memo.slice(2 + 50);
    expect(saltHex).toBe("ff010203040506");
  });

  it("throws on invalid ULID length", () => {
    expect(() =>
      encodeMemoV1({
        type: "invoice",
        issuerTag: TAG,
        ulid: "TOO_SHORT",
      }),
    ).toThrow("26 characters");
  });

  it("throws on wrong salt length", () => {
    expect(() =>
      encodeMemoV1({
        type: "invoice",
        issuerTag: TAG,
        ulid: "01MASW9NF6YW40J40H289H858P",
        salt: new Uint8Array(5),
      }),
    ).toThrow("7 bytes");
  });

  it("throws on issuerTag > 2^64-1", () => {
    expect(() =>
      encodeMemoV1({
        type: "invoice",
        issuerTag: 2n ** 64n,
        ulid: "01MASW9NF6YW40J40H289H858P",
      }),
    ).toThrow("uint64");
  });

  it("throws on negative issuerTag", () => {
    expect(() =>
      encodeMemoV1({
        type: "invoice",
        issuerTag: -1n,
        ulid: "01MASW9NF6YW40J40H289H858P",
      }),
    ).toThrow("uint64");
  });

  it("throws on invalid type", () => {
    expect(() =>
      encodeMemoV1({
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        type: "bogus" as any,
        issuerTag: TAG,
        ulid: "01MASW9NF6YW40J40H289H858P",
      }),
    ).toThrow("Invalid memo type");
  });

  it('accepts salt: "random" and produces non-zero salt', () => {
    const memo = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
      salt: "random",
    });
    expect(memo).toMatch(/^0x[0-9a-f]{64}$/);
    const saltHex = memo.slice(2 + 50);
    // random salt is extremely unlikely to be all zeros
    expect(saltHex).not.toBe("00000000000000");
  });

  it('produces different memos with salt: "random"', () => {
    const a = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
      salt: "random",
    });
    const b = encodeMemoV1({
      type: "invoice",
      issuerTag: TAG,
      ulid: "01MASW9NF6YW40J40H289H858P",
      salt: "random",
    });
    expect(a).not.toBe(b);
  });
});

describe("randomSalt", () => {
  it("returns 7 bytes", () => {
    const salt = randomSalt();
    expect(salt).toBeInstanceOf(Uint8Array);
    expect(salt.length).toBe(7);
  });

  it("returns different values on each call", () => {
    const a = randomSalt();
    const b = randomSalt();
    expect(a).not.toEqual(b);
  });
});
