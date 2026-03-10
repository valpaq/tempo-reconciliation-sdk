import type { MemoV1 } from "../types";
import {
  CODE_TO_TYPE,
  MEMO_BYTES,
  ISSUER_TAG_OFFSET,
  ISSUER_TAG_SIZE,
  ID16_OFFSET,
  ID16_SIZE,
  SALT_OFFSET,
  SALT_SIZE,
} from "./constants";
import { bytes16ToUlid } from "./ulid";

function hexToBytes(hex: string): Uint8Array | null {
  if (hex.length % 2 !== 0) return null;
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    const byte = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    if (Number.isNaN(byte)) return null;
    bytes[i] = byte;
  }
  return bytes;
}

function hasNoControlChars(bytes: Uint8Array): boolean {
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i]!;
    if (b < 0x09) return false;
    if (b === 0x0b || b === 0x0c) return false;
    if (b >= 0x0e && b <= 0x1f) return false;
    if (b === 0x7f) return false;
  }
  return true;
}

function tryDecodeUtf8(bytes: Uint8Array): string | null {
  if (!hasNoControlChars(bytes)) return null;
  try {
    const decoder = new TextDecoder("utf-8", { fatal: true });
    return decoder.decode(bytes);
  } catch {
    return null;
  }
}

/**
 * Decode a bytes32 memo as plain UTF-8 text.
 *
 * Handles both left- and right-zero-padded strings. Returns `null` if the
 * bytes are not printable UTF-8 after stripping zero padding.
 *
 * @param memoRaw - `0x`-prefixed 66-character hex string
 * @returns Decoded text string, or `null` if not valid printable UTF-8
 * @example
 * ```ts
 * const text = decodeMemoText("0x494e562d30303100000000000000000000000000000000000000000000000000");
 * // text === "INV-001"
 * ```
 */
export function decodeMemoText(memoRaw: `0x${string}`): string | null {
  const hexStr = memoRaw.slice(2);
  if (hexStr.length !== MEMO_BYTES * 2) return null;

  const bytes = hexToBytes(hexStr);
  if (!bytes) return null;

  // try right-padded first (strip trailing zeros)
  let end = bytes.length;
  while (end > 0 && bytes[end - 1] === 0) end--;
  if (end === 0) return null;

  let start = 0;
  while (start < bytes.length && bytes[start] === 0) start++;

  // right-padded: text at the start, zeros at the end
  if (start === 0 && end < bytes.length) {
    return tryDecodeUtf8(bytes.slice(0, end));
  }

  // left-padded: zeros at the start, text at the end
  if (start > 0 && end === bytes.length) {
    return tryDecodeUtf8(bytes.slice(start));
  }

  // no padding (all 32 bytes are content)
  if (start === 0 && end === bytes.length) {
    return tryDecodeUtf8(bytes);
  }

  // zeros on both sides — non-zero middle
  return tryDecodeUtf8(bytes.slice(start, end));
}

/**
 * Decode a bytes32 memo, trying v1 structured format first, then UTF-8 text.
 *
 * @param memoRaw - `0x`-prefixed 66-character hex string
 * @returns `MemoV1` if v1 structured format, `string` if UTF-8 text, or `null`
 * @example
 * ```ts
 * const decoded = decodeMemo(event.memoRaw);
 * if (decoded && typeof decoded === "object") {
 *   console.log(decoded.ulid); // MemoV1
 * } else if (typeof decoded === "string") {
 *   console.log(decoded); // plain text memo
 * }
 * ```
 */
export function decodeMemo(memoRaw: `0x${string}`): MemoV1 | string | null {
  const v1 = decodeMemoV1(memoRaw);
  if (v1) return v1;

  const text = decodeMemoText(memoRaw);
  if (text) return text;

  return null;
}

/**
 * Decode a bytes32 hex string as a v1 structured memo.
 *
 * Returns `null` (never throws) for any input that doesn't match the v1 layout,
 * including unknown type codes, wrong lengths, and invalid ULID bytes.
 *
 * @param memoRaw - `0x`-prefixed 66-character hex string
 * @returns Decoded `MemoV1` object, or `null` if not a valid v1 memo
 * @example
 * ```ts
 * const memo = decodeMemoV1(event.memoRaw);
 * if (memo) {
 *   console.log(memo.t, memo.ulid, memo.issuerTag);
 * }
 * ```
 */
export function decodeMemoV1(memoRaw: `0x${string}`): MemoV1 | null {
  const hexStr = memoRaw.slice(2);
  if (hexStr.length !== MEMO_BYTES * 2) {
    return null;
  }

  const buf = hexToBytes(hexStr);
  if (!buf) return null;

  const typeCode = buf[0]!;
  const t = CODE_TO_TYPE[typeCode];
  if (!t) {
    return null;
  }

  let issuerTag = 0n;
  for (let i = 0; i < ISSUER_TAG_SIZE; i++) {
    issuerTag = (issuerTag << 8n) | BigInt(buf[ISSUER_TAG_OFFSET + i]!);
  }

  const id16 = buf.slice(ID16_OFFSET, ID16_OFFSET + ID16_SIZE);
  const salt = buf.slice(SALT_OFFSET, SALT_OFFSET + SALT_SIZE);

  const ulid = bytes16ToUlid(id16);

  return {
    v: 1,
    t,
    issuerTag,
    ulid,
    id16,
    salt,
    raw: memoRaw,
  };
}

/**
 * Type guard: checks whether a decoded memo is a structured `MemoV1` object.
 *
 * @param memo - Return value from `decodeMemo()` or `decodeMemoV1()`
 * @returns `true` if `memo` is a `MemoV1` object with `v === 1`
 * @example
 * ```ts
 * const decoded = decodeMemo(event.memoRaw);
 * if (isMemoV1(decoded)) {
 *   console.log(decoded.ulid); // narrowed to MemoV1
 * }
 * ```
 */
export function isMemoV1(memo: MemoV1 | string | null): memo is MemoV1 {
  return memo !== null && typeof memo === "object" && "v" in memo && memo.v === 1;
}
