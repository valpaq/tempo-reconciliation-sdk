import { describe, it, expect } from "vitest";
import { decodeMemoV1, decodeMemoText, decodeMemo, isMemoV1 } from "../../src/memo/decode";
import { encodeMemoV1 } from "../../src/memo/encode";
import { issuerTagFromNamespace } from "../../src/memo/issuer-tag";

describe("decodeMemoV1", () => {
  const TAG = issuerTagFromNamespace("test-app");
  const ULID = "01MASW9NF6YW40J40H289H858P";

  it("roundtrips with encodeMemoV1", () => {
    const encoded = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: ULID });
    const decoded = decodeMemoV1(encoded);

    expect(decoded).not.toBeNull();
    expect(decoded!.v).toBe(1);
    expect(decoded!.t).toBe("invoice");
    expect(decoded!.issuerTag).toBe(TAG);
    expect(decoded!.ulid).toBe(ULID);
    expect(decoded!.raw).toBe(encoded);
  });

  it("returns null for all zeros", () => {
    expect(
      decodeMemoV1("0x0000000000000000000000000000000000000000000000000000000000000000"),
    ).toBeNull();
  });

  it("returns null for reserved type 0x00", () => {
    // byte 0 = 0x00 (reserved type), rest is valid-looking data
    const tampered =
      "0x00fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000" as `0x${string}`;
    expect(decodeMemoV1(tampered)).toBeNull();
  });

  it("returns null for reserved type 0x7", () => {
    // byte 0 = 0x07 (reserved type)
    const tampered =
      "0x07fc7c8482914a04e801a2b3c4d5e6f708091011121314151600000000000000" as `0x${string}`;
    expect(decodeMemoV1(tampered)).toBeNull();
  });

  it("returns null for short input", () => {
    expect(decodeMemoV1("0x11fc7c" as `0x${string}`)).toBeNull();
  });

  it("returns null for long input", () => {
    expect(
      decodeMemoV1(
        "0x01fc7c8482914a04e801a2b3c4d5e6f7080910111213141516000000000000000000" as `0x${string}`,
      ),
    ).toBeNull();
  });

  it("returns null for empty string", () => {
    expect(decodeMemoV1("0x" as `0x${string}`)).toBeNull();
  });

  it("preserves salt bytes", () => {
    const salt = new Uint8Array([0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00]);
    const encoded = encodeMemoV1({ type: "batch", issuerTag: TAG, ulid: ULID, salt });
    const decoded = decodeMemoV1(encoded);

    expect(decoded).not.toBeNull();
    expect(decoded!.salt).toEqual(salt);
  });

  it("decodes unknown issuerTag without crashing", () => {
    // New layout: byte 0 = type (0x01=invoice), bytes 1-8 = issuerTag (deadbeefdeadbeef),
    // bytes 9-24 = id16 of '01MASW9NF6YW40J40H289H858P' (01a2b3c4d5e6f7080910111213141516),
    // bytes 25-31 = salt zeros (00000000000000)
    // total = 1 + 8 + 16 + 7 = 32 bytes = 64 hex chars
    const memo = "0x01deadbeefdeadbeef01a2b3c4d5e6f7080910111213141516000000000000 00".replace(
      / /g,
      "",
    ) as `0x${string}`;
    const decoded = decodeMemoV1(memo);
    expect(decoded).not.toBeNull();
    expect(decoded!.issuerTag).toBe(0xdeadbeefdeadbeefn);
  });
});

// helper: encode a UTF-8 string as bytes32 hex (right-padded with zeros)
function textToBytes32Hex(text: string): `0x${string}` {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(text);
  const buf = new Uint8Array(32);
  buf.set(bytes.slice(0, 32));
  let hex = "0x";
  for (const b of buf) {
    hex += b.toString(16).padStart(2, "0");
  }
  return hex as `0x${string}`;
}

// helper: encode a UTF-8 string as bytes32 hex (left-padded with zeros)
function textToBytes32HexLeft(text: string): `0x${string}` {
  const encoder = new TextEncoder();
  const bytes = encoder.encode(text);
  const buf = new Uint8Array(32);
  buf.set(bytes.slice(0, 32), 32 - bytes.length);
  let hex = "0x";
  for (const b of buf) {
    hex += b.toString(16).padStart(2, "0");
  }
  return hex as `0x${string}`;
}

describe("decodeMemoText", () => {
  it("decodes UTF-8 string right-padded with zeros", () => {
    const hex = textToBytes32Hex("PAY-595079");
    expect(decodeMemoText(hex)).toBe("PAY-595079");
  });

  it("decodes short string", () => {
    const hex = textToBytes32Hex("tx1352");
    expect(decodeMemoText(hex)).toBe("tx1352");
  });

  it("returns null for all zeros", () => {
    expect(
      decodeMemoText("0x0000000000000000000000000000000000000000000000000000000000000000"),
    ).toBeNull();
  });

  it("returns null for wrong length", () => {
    expect(decodeMemoText("0xaabb" as `0x${string}`)).toBeNull();
  });

  it('decodes real testnet memo "dropsnap"', () => {
    // 0x64726f70736e61700d + zeros — from Moderato testnet block 6504870
    const hex =
      "0x64726f70736e61700d0000000000000000000000000000000000000000000000" as `0x${string}`;
    const text = decodeMemoText(hex);
    expect(text).toBe("dropsnap\r");
  });

  it("handles string that fills all 32 bytes", () => {
    const hex = textToBytes32Hex("abcdefghijklmnopqrstuvwxyz012345");
    expect(decodeMemoText(hex)).toBe("abcdefghijklmnopqrstuvwxyz012345");
  });

  it("decodes left-padded text (zeros on the left)", () => {
    const hex = textToBytes32HexLeft("dinner001");
    expect(decodeMemoText(hex)).toBe("dinner001");
  });

  it('decodes left-padded "pay-coffee001"', () => {
    const hex = textToBytes32HexLeft("pay-coffee001");
    expect(decodeMemoText(hex)).toBe("pay-coffee001");
  });

  it('decodes left-padded "Salary - Jan 2026"', () => {
    const hex = textToBytes32HexLeft("Salary - Jan 2026");
    expect(decodeMemoText(hex)).toBe("Salary - Jan 2026");
  });

  it("decodes real testnet left-padded memo", () => {
    // 0x000000000000000000000000000000000000000000000064696e6e6572303031
    // = "dinner001" left-padded
    const hex =
      "0x000000000000000000000000000000000000000000000064696e6e6572303031" as `0x${string}`;
    expect(decodeMemoText(hex)).toBe("dinner001");
  });

  it("decodes left-padded memo with leading zero in text byte", () => {
    // "groceries001" left-padded
    const hex = textToBytes32HexLeft("groceries001");
    expect(decodeMemoText(hex)).toBe("groceries001");
  });

  it("decodes text with zeros on both sides", () => {
    // "hi" (0x6869) with 10 zero bytes before and 20 zero bytes after
    const hex = ("0x" + "00".repeat(10) + "6869" + "00".repeat(20)) as `0x${string}`;
    expect(decodeMemoText(hex)).toBe("hi");
  });
});

describe("decodeMemo", () => {
  const TAG = issuerTagFromNamespace("test-app");
  const ULID = "01MASW9NF6YW40J40H289H858P";

  it("returns MemoV1 for valid v1 memo", () => {
    const encoded = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: ULID });
    const result = decodeMemo(encoded);
    expect(result).not.toBeNull();
    expect(typeof result).toBe("object");
    expect((result as { v: number }).v).toBe(1);
  });

  it("returns string for UTF-8 memo", () => {
    const hex = textToBytes32Hex("PAY-595079");
    const result = decodeMemo(hex);
    expect(result).toBe("PAY-595079");
  });

  it("returns null for all zeros", () => {
    expect(
      decodeMemo("0x0000000000000000000000000000000000000000000000000000000000000000"),
    ).toBeNull();
  });

  it("prefers v1 decode over text", () => {
    // a valid v1 memo should return MemoV1, not its text representation
    const encoded = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: ULID });
    const result = decodeMemo(encoded);
    expect(typeof result).toBe("object");
  });

  it("returns string for left-padded text memo", () => {
    const hex = textToBytes32HexLeft("rent001");
    const result = decodeMemo(hex);
    expect(result).toBe("rent001");
  });
});

describe("isMemoV1", () => {
  const TAG = issuerTagFromNamespace("test-app");
  const ULID = "01MASW9NF6YW40J40H289H858P";

  it("returns true for a valid MemoV1 object", () => {
    const encoded = encodeMemoV1({ type: "invoice", issuerTag: TAG, ulid: ULID });
    const decoded = decodeMemoV1(encoded);
    expect(isMemoV1(decoded)).toBe(true);
  });

  it("returns false for null", () => {
    expect(isMemoV1(null)).toBe(false);
  });

  it("returns false for a string", () => {
    expect(isMemoV1("PAY-595079")).toBe(false);
  });

  it("returns false for undefined", () => {
    // isMemoV1 accepts MemoV1 | string | null; cast to test the guard
    expect(isMemoV1(undefined as unknown as null)).toBe(false);
  });
});
