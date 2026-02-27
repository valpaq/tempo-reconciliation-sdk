import type { ExplorerOptions, AddressMetadata, BalancesResponse, HistoryResponse } from "../types";

const DEFAULT_BASE = "https://explore.tempo.xyz/api";

/**
 * REST client for the Tempo Explorer API.
 *
 * Provides methods to query address metadata, token balances, and transaction
 * history from the Tempo block explorer at `https://explore.tempo.xyz/api`.
 */
export class ExplorerClient {
  private readonly base: string;
  private readonly fetchFn: typeof globalThis.fetch;

  /**
   * @param options - Optional base URL override and custom fetch function
   */
  constructor(options?: ExplorerOptions) {
    this.base = (options?.baseUrl ?? DEFAULT_BASE).replace(/\/+$/, "");
    this.fetchFn = options?.fetch ?? globalThis.fetch;
  }

  /**
   * Fetch on-chain metadata for an address (account type, tx count, timestamps).
   *
   * @param address - Hex address to look up
   * @returns Address metadata including type, nonce, and creation info
   * @throws `Error` with `"Explorer API <status>"` on non-2xx response
   */
  getMetadata(address: string): Promise<AddressMetadata> {
    return this.fetchJson(`/address/metadata/${address}`);
  }

  /**
   * Fetch TIP-20 token balances for an address.
   *
   * @param address - Hex address to look up
   * @returns Array of token balances with token address and raw amount
   * @throws `Error` with `"Explorer API <status>"` on non-2xx response
   */
  getBalances(address: string): Promise<BalancesResponse> {
    return this.fetchJson(`/address/balances/${address}`);
  }

  /**
   * Fetch paginated transaction history for an address.
   *
   * @param address - Hex address to look up
   * @param opts - Optional `limit` (items per page) and `offset` (pagination cursor)
   * @returns Paginated list of transactions
   * @throws `Error` with `"Explorer API <status>"` on non-2xx response
   */
  getHistory(
    address: string,
    opts?: { limit?: number; offset?: number },
  ): Promise<HistoryResponse> {
    const params = new URLSearchParams();
    if (opts?.limit != null) params.set("limit", String(opts.limit));
    if (opts?.offset != null) params.set("offset", String(opts.offset));
    const qs = params.toString();
    return this.fetchJson(`/address/history/${address}${qs ? `?${qs}` : ""}`);
  }

  private async fetchJson<T>(path: string): Promise<T> {
    const res = await this.fetchFn(`${this.base}${path}`, {
      signal: AbortSignal.timeout(30_000),
    });
    if (!res.ok) throw new Error(`Explorer API ${res.status}: ${path}`);
    return (await res.json()) as T;
  }
}

/**
 * Create an ExplorerClient for the Tempo Explorer REST API.
 *
 * @param options - Optional base URL override and custom fetch function
 * @returns Configured ExplorerClient instance
 * @example
 * ```ts
 * const explorer = createExplorerClient();
 * const meta = await explorer.getMetadata("0x1234...");
 * const balances = await explorer.getBalances("0x1234...");
 * ```
 */
export function createExplorerClient(options?: ExplorerOptions): ExplorerClient {
  return new ExplorerClient(options);
}
