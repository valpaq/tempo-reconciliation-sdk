import { describe, it, expect } from "vitest";
import { encodeMemoV1, decodeMemoV1, issuerTagFromNamespace } from "../../src/memo/index";
import vectors from "../../../spec/vectors.json";

describe("spec/vectors.json", () => {
  describe("positive vectors (roundtrip)", () => {
    for (const v of vectors.positive) {
      it(v.name, () => {
        const tag = issuerTagFromNamespace(v.namespace);
        expect(tag).toBe(BigInt(v.issuerTag));

        const salt =
          v.saltHex === "00000000000000"
            ? undefined
            : Uint8Array.from(v.saltHex.match(/.{2}/g)!.map((h: string) => parseInt(h, 16)));

        const encoded = encodeMemoV1({
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          type: v.type as any,
          issuerTag: BigInt(v.issuerTag),
          ulid: v.ulid,
          salt,
        });
        expect(encoded).toBe(v.memoRaw);

        const decoded = decodeMemoV1(v.memoRaw as `0x${string}`);
        expect(decoded).not.toBeNull();
        expect(decoded!.v).toBe(1);
        expect(decoded!.t).toBe(v.type);
        expect(decoded!.issuerTag).toBe(BigInt(v.issuerTag));
        expect(decoded!.ulid).toBe(v.ulid);
        expect(decoded!.raw).toBe(v.memoRaw);
        const expectedSalt = Uint8Array.from(
          (v.saltHex as string).match(/.{2}/g)!.map((h: string) => parseInt(h, 16)),
        );
        expect(decoded!.salt).toEqual(expectedSalt);
      });
    }
  });

  describe("negative vectors (decode → null)", () => {
    for (const v of vectors.negative) {
      it(v.name, () => {
        const result = decodeMemoV1(v.memoRaw as `0x${string}`);
        expect(result).toBeNull();
      });
    }
  });
});
