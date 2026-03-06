import { describe, it, expect } from "vitest";
import { encodeMemoV1, decodeMemoV1, issuerTagFromNamespace } from "../../src/memo/index";
import type { MemoType } from "../../src/types";
import vectorsRaw from "../../../spec/vectors.json";

type PositiveVector = {
  name: string;
  type: MemoType;
  namespace: string;
  issuerTag: string;
  ulid: string;
  saltHex: string;
  memoRaw: `0x${string}`;
};

type NegativeVector = {
  name: string;
  memoRaw: `0x${string}`;
};

const vectors = vectorsRaw as { positive: PositiveVector[]; negative: NegativeVector[] };

describe("spec/vectors.json", () => {
  describe("positive vectors (roundtrip)", () => {
    for (const v of vectors.positive) {
      it(v.name, () => {
        const tag = issuerTagFromNamespace(v.namespace);
        expect(tag).toBe(BigInt(v.issuerTag));

        const salt =
          v.saltHex === "00000000000000"
            ? undefined
            : Uint8Array.from(v.saltHex.match(/.{2}/g)!.map((h) => parseInt(h, 16)));

        const encoded = encodeMemoV1({
          type: v.type,
          issuerTag: BigInt(v.issuerTag),
          ulid: v.ulid,
          salt,
        });
        expect(encoded).toBe(v.memoRaw);

        const decoded = decodeMemoV1(v.memoRaw);
        expect(decoded).not.toBeNull();
        expect(decoded!.v).toBe(1);
        expect(decoded!.t).toBe(v.type);
        expect(decoded!.issuerTag).toBe(BigInt(v.issuerTag));
        expect(decoded!.ulid).toBe(v.ulid);
        expect(decoded!.raw).toBe(v.memoRaw);
        const expectedSalt = Uint8Array.from(v.saltHex.match(/.{2}/g)!.map((h) => parseInt(h, 16)));
        expect(decoded!.salt).toEqual(expectedSalt);
      });
    }
  });

  describe("negative vectors (decode → null)", () => {
    for (const v of vectors.negative) {
      it(v.name, () => {
        const result = decodeMemoV1(v.memoRaw);
        expect(result).toBeNull();
      });
    }
  });
});
