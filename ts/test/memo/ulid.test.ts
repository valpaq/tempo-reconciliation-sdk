import { describe, it, expect } from "vitest";
import { ulidToBytes16, bytes16ToUlid } from "../../src/memo/ulid";

describe("ulidToBytes16 / bytes16ToUlid", () => {
  it("roundtrips a known ULID", () => {
    const ulid = "01MASW9NF6YW40J40H289H858P";
    const bytes = ulidToBytes16(ulid);
    expect(bytes.length).toBe(16);
    const back = bytes16ToUlid(bytes);
    expect(back).toBe(ulid);
  });

  it("roundtrips all-zero ULID", () => {
    const ulid = "00000000000000000000000000";
    const bytes = ulidToBytes16(ulid);
    expect(bytes).toEqual(new Uint8Array(16));
    expect(bytes16ToUlid(bytes)).toBe(ulid);
  });

  it("roundtrips max ULID", () => {
    const ulid = "7ZZZZZZZZZZZZZZZZZZZZZZZZZ";
    const bytes = ulidToBytes16(ulid);
    expect(bytes16ToUlid(bytes)).toBe(ulid);
  });

  it("throws on wrong length", () => {
    expect(() => ulidToBytes16("SHORT")).toThrow("26 characters");
  });

  it("throws on invalid character", () => {
    expect(() => ulidToBytes16("01MASW9NF6YW40J40H289H8U8P")).toThrow("Invalid ULID character");
  });

  it("bytes16ToUlid throws on wrong length", () => {
    expect(() => bytes16ToUlid(new Uint8Array(15))).toThrow("16 bytes");
  });

  it("handles lowercase input", () => {
    const upper = "01MASW9NF6YW40J40H289H858P";
    const lower = upper.toLowerCase();
    const bytesUpper = ulidToBytes16(upper);
    const bytesLower = ulidToBytes16(lower);
    expect(bytesUpper).toEqual(bytesLower);
  });
});
