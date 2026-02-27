import type { EncodeMemoV1Params } from "../types";
import {
  TYPE_CODES,
  MEMO_BYTES,
  ISSUER_TAG_OFFSET,
  ISSUER_TAG_SIZE,
  ID16_OFFSET,
  SALT_OFFSET,
  SALT_SIZE,
} from "./constants";
import { ulidToBytes16 } from "./ulid";

/**
 * Generate 7 cryptographically random bytes for the salt field.
 *
 * @returns 7-byte Uint8Array from `crypto.getRandomValues`
 * @example
 * ```ts
 * const salt = randomSalt();
 * const memo = encodeMemoV1({ type: "invoice", issuerTag, ulid, salt });
 * ```
 */
export function randomSalt(): Uint8Array {
  const buf = new Uint8Array(SALT_SIZE);
  crypto.getRandomValues(buf);
  return buf;
}

/**
 * Encode memo fields into a 32-byte hex string per TEMPO-RECONCILE-MEMO-001.
 *
 * Layout: `[type:1][issuerTag:8][id16:16][salt:7]` = 32 bytes.
 *
 * @param params - Memo fields: type, issuerTag, ulid, and optional salt
 * @returns `0x`-prefixed 66-character hex string (32 bytes)
 * @throws If `params.type` is not a valid memo type code
 * @throws If `params.salt` is provided and not exactly 7 bytes
 * @example
 * ```ts
 * const memo = encodeMemoV1({
 *   type: "invoice",
 *   issuerTag: issuerTagFromNamespace("my-app"),
 *   ulid: "01MASW9NF6YW40J40H289H858P",
 * });
 * ```
 */
export function encodeMemoV1(params: EncodeMemoV1Params): `0x${string}` {
  const typeCode = TYPE_CODES[params.type];
  if (typeCode === undefined) {
    throw new Error(`Invalid memo type: ${params.type}`);
  }

  const id16 = ulidToBytes16(params.ulid);

  const salt = params.salt === "random" ? randomSalt() : (params.salt ?? new Uint8Array(SALT_SIZE));
  if (salt.length !== SALT_SIZE) {
    throw new Error(`salt must be ${SALT_SIZE} bytes, got ${salt.length}`);
  }

  const buf = new Uint8Array(MEMO_BYTES);

  buf[0] = typeCode;

  const tag = params.issuerTag;
  if (tag < 0n || tag > 0xffffffffffffffffn) {
    throw new Error(`issuerTag must be a uint64 [0, 2^64-1], got ${tag}`);
  }
  for (let i = 0; i < ISSUER_TAG_SIZE; i++) {
    buf[ISSUER_TAG_OFFSET + i] = Number((tag >> BigInt((ISSUER_TAG_SIZE - 1 - i) * 8)) & 0xffn);
  }

  buf.set(id16, ID16_OFFSET);
  buf.set(salt, SALT_OFFSET);

  let hex = "0x";
  for (let i = 0; i < MEMO_BYTES; i++) {
    hex += buf[i]!.toString(16).padStart(2, "0");
  }

  return hex as `0x${string}`;
}
