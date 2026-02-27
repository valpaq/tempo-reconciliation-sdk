import { keccak_256 } from "@noble/hashes/sha3";

/**
 * Derive an issuer tag from a namespace string.
 *
 * Computes `keccak256(namespace)` and takes the first 8 bytes as a `uint64` bigint.
 * Deterministic: the same namespace always produces the same tag.
 *
 * @param namespace - Application namespace (e.g. `"my-app"`, `"acme-payments"`)
 * @returns `uint64` bigint (8 bytes of keccak256 hash)
 * @example
 * ```ts
 * const tag = issuerTagFromNamespace("my-app");
 * const memo = encodeMemoV1({ type: "invoice", issuerTag: tag, ulid });
 * ```
 */
export function issuerTagFromNamespace(namespace: string): bigint {
  const encoded = new TextEncoder().encode(namespace);
  const hash = keccak_256(encoded);
  let tag = 0n;
  for (let i = 0; i < 8; i++) {
    tag = (tag << 8n) | BigInt(hash[i]!);
  }
  return tag;
}
