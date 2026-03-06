import { NoncePool } from "../src/pool";
import type { NoncePoolOptions } from "../src/types";

export const defaultOpts: NoncePoolOptions = {
  address: "0x1234567890abcdef1234567890abcdef12345678",
  rpcUrl: "https://rpc.moderato.tempo.xyz",
};

export async function createPool(overrides?: Partial<NoncePoolOptions>): Promise<NoncePool> {
  const pool = new NoncePool({ ...defaultOpts, ...overrides });
  await pool.init();
  return pool;
}
