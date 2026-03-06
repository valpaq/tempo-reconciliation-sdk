const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

const DECODE_MAP = new Map<string, number>();
for (let i = 0; i < CROCKFORD.length; i++) {
  const c = CROCKFORD[i]!;
  DECODE_MAP.set(c, i);
  DECODE_MAP.set(c.toLowerCase(), i);
}
// Crockford aliases
DECODE_MAP.set("O", 0);
DECODE_MAP.set("o", 0);
DECODE_MAP.set("I", 1);
DECODE_MAP.set("i", 1);
DECODE_MAP.set("L", 1);
DECODE_MAP.set("l", 1);

/**
 * Convert a ULID string to its 16-byte binary representation.
 *
 * @param ulid - 26-character Crockford base32 ULID string
 * @returns 16-byte Uint8Array
 * @throws If `ulid` is not exactly 26 characters
 * @throws If `ulid` contains invalid Crockford base32 characters
 * @example
 * ```ts
 * const bytes = ulidToBytes16("01MASW9NF6YW40J40H289H858P");
 * // bytes.length === 16
 * ```
 */
export function ulidToBytes16(ulid: string): Uint8Array {
  if (ulid.length !== 26) {
    throw new Error(`ULID must be 26 characters, got ${ulid.length}`);
  }

  const bytes = new Uint8Array(16);

  let bitBuffer = 0n;
  for (let i = 0; i < 26; i++) {
    const val = DECODE_MAP.get(ulid[i]!);
    if (val === undefined) {
      throw new Error(`Invalid ULID character: ${ulid[i]}`);
    }
    bitBuffer = (bitBuffer << 5n) | BigInt(val);
  }

  for (let i = 15; i >= 0; i--) {
    bytes[i] = Number(bitBuffer & 0xffn);
    bitBuffer >>= 8n;
  }

  return bytes;
}

/**
 * Convert a 16-byte binary value to a ULID string.
 *
 * @param id16 - 16-byte Uint8Array (from memo `id16` field)
 * @returns 26-character Crockford base32 ULID string
 * @throws If `id16` is not exactly 16 bytes
 * @example
 * ```ts
 * const ulid = bytes16ToUlid(memo.id16);
 * // ulid === "01MASW9NF6YW40J40H289H858P"
 * ```
 */
export function bytes16ToUlid(id16: Uint8Array): string {
  if (id16.length !== 16) {
    throw new Error(`id16 must be 16 bytes, got ${id16.length}`);
  }

  let bitBuffer = 0n;
  for (let i = 0; i < 16; i++) {
    bitBuffer = (bitBuffer << 8n) | BigInt(id16[i]!);
  }

  const chars: string[] = [];
  for (let i = 0; i < 26; i++) {
    chars.unshift(CROCKFORD[Number(bitBuffer & 0x1fn)]!);
    bitBuffer >>= 5n;
  }

  return chars.join("");
}
