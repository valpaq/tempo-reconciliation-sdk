import type { PublicClient } from "viem";
import { INONCE_ABI, NONCE_PRECOMPILE } from "./constants";

/**
 * Query the Nonce precompile for a specific (address, key) pair.
 *
 * @param client - viem PublicClient instance
 * @param address - Account address to query nonce for
 * @param key - Nonce key (lane index or maxUint256 for expiring)
 * @returns Current nonce value as bigint
 */
export async function getNonceFromPrecompile(
  client: PublicClient,
  address: `0x${string}`,
  key: bigint,
): Promise<bigint> {
  return client.readContract({
    address: NONCE_PRECOMPILE,
    abi: INONCE_ABI,
    functionName: "getNonce",
    args: [address, key],
  });
}

/**
 * Query the protocol nonce (key=0) via eth_getTransactionCount.
 * Utility for callers who need the standard EVM nonce — not used by NoncePool internally.
 *
 * @param client - viem PublicClient instance
 * @param address - Account address
 * @returns Pending nonce as bigint
 */
export async function getProtocolNonce(
  client: PublicClient,
  address: `0x${string}`,
): Promise<bigint> {
  const count = await client.getTransactionCount({
    address,
    blockTag: "pending",
  });
  return BigInt(count);
}
