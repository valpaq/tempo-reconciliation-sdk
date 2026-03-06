/** Tempo Nonce precompile address (ASCII "NONCE" zero-padded to 20 bytes). */
export const NONCE_PRECOMPILE = "0x4e4F4E4345000000000000000000000000000000" as const;

/** Max uint256 — used as nonceKey for TIP-1009 expiring nonce mode. */
export const MAX_UINT256 = 2n ** 256n - 1n;

/** Moderato testnet chain ID. */
export const MODERATO_CHAIN_ID = 42431;

/** Default number of parallel lanes. */
export const DEFAULT_LANES = 4;

/** Default reservation TTL before auto-expiry (ms). */
export const DEFAULT_RESERVATION_TTL_MS = 30_000;

/** Default validBefore offset for expiring mode (seconds). */
export const DEFAULT_VALID_BEFORE_OFFSET_S = 30;

/**
 * INonce precompile ABI.
 * `getNonce(address owner, uint256 key) returns (uint64)`
 */
export const INONCE_ABI = [
  {
    type: "function",
    name: "getNonce",
    inputs: [
      { name: "owner", type: "address" },
      { name: "key", type: "uint256" },
    ],
    outputs: [{ name: "", type: "uint64" }],
    stateMutability: "view",
  },
] as const;
