import { describe, it, expect, vi, beforeEach, afterEach, type MockInstance } from "vitest";
import { createExplorerClient } from "../../src/explorer/client";

const MOCK_METADATA = {
  address: "0x51881fed631dae3f998dad2cf0c13e0a932cbb11",
  chainId: 42431,
  accountType: "empty",
  txCount: 134,
  lastActivityTimestamp: 1772180622,
  createdTimestamp: 1768127571,
  createdTxHash: "0x3b12cc089242be1274fd072c116cd35c8cb2908e89998e3ae5c6bea3e1839586",
  createdBy: "0x51881fed631dae3f998dad2cf0c13e0a932cbb11",
};

const MOCK_BALANCES = {
  balances: [
    {
      token: "0x20c0000000000000000000000000000000000000",
      balance: "1799109000000",
      name: "PathUSD",
      symbol: "PathUSD",
      currency: "USD",
      decimals: 6,
    },
  ],
};

const MOCK_HISTORY = {
  transactions: [
    {
      hash: "0xba01fd25c190087f10d6d6d921f2d4a3e0e7aafd21e92cbb7f56851060e3d3ba",
      blockNumber: "0x6341a6",
      timestamp: 1772180622,
      from: "0x51881FeD631dAe3f998daD2cf0C13e0A932CbB11",
      to: "0x20C0000000000000000000000000000000000000",
      value: "0x0",
      status: "success",
      gasUsed: "0x82ccb",
      effectiveGasPrice: "0x4a817c802",
      knownEvents: [
        {
          type: "send",
          note: "dropsnap",
          parts: [
            { type: "action", value: "Send" },
            {
              type: "amount",
              value: { token: "0x20c0...", value: "50000000", decimals: 6, symbol: "PathUSD" },
            },
          ],
          meta: { from: "0x5188...", to: "0x4489..." },
        },
      ],
    },
  ],
  total: 166,
  offset: 0,
  limit: 2,
  hasMore: true,
  countCapped: false,
  error: null,
};

describe("createExplorerClient", () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let fetchSpy: MockInstance<any>;

  beforeEach(() => {
    fetchSpy = vi.spyOn(globalThis, "fetch");
  });

  afterEach(() => {
    fetchSpy.mockRestore();
  });

  it("fetches address metadata", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_METADATA), { status: 200 }));
    const client = createExplorerClient();
    const meta = await client.getMetadata("0x5188");
    expect(meta.chainId).toBe(42431);
    expect(meta.txCount).toBe(134);
    expect(fetchSpy.mock.calls[0]![0]).toBe(
      "https://explore.tempo.xyz/api/address/metadata/0x5188",
    );
  });

  it("fetches token balances", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_BALANCES), { status: 200 }));
    const client = createExplorerClient();
    const bal = await client.getBalances("0x5188");
    expect(bal.balances).toHaveLength(1);
    expect(bal.balances[0]!.symbol).toBe("PathUSD");
  });

  it("fetches transaction history", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_HISTORY), { status: 200 }));
    const client = createExplorerClient();
    const hist = await client.getHistory("0x5188");
    expect(hist.transactions).toHaveLength(1);
    expect(hist.total).toBe(166);
    expect(hist.hasMore).toBe(true);
  });

  it("passes limit and offset as query params", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_HISTORY), { status: 200 }));
    const client = createExplorerClient();
    await client.getHistory("0x5188", { limit: 10, offset: 20 });
    const url = fetchSpy.mock.calls[0]![0] as string;
    expect(url).toContain("limit=10");
    expect(url).toContain("offset=20");
  });

  it("passes limit=0 and offset=0 as query params", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_HISTORY), { status: 200 }));
    const client = createExplorerClient();
    await client.getHistory("0x5188", { limit: 0, offset: 0 });
    const url = fetchSpy.mock.calls[0]![0] as string;
    expect(url).toContain("limit=0");
    expect(url).toContain("offset=0");
  });

  it("throws on non-ok response", async () => {
    fetchSpy.mockResolvedValueOnce(new Response("Not Found", { status: 404 }));
    const client = createExplorerClient();
    await expect(client.getMetadata("0xbad")).rejects.toThrow("Explorer API 404");
  });

  it("uses custom baseUrl", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_METADATA), { status: 200 }));
    const client = createExplorerClient({ baseUrl: "https://custom.api/v1" });
    await client.getMetadata("0x5188");
    expect(fetchSpy.mock.calls[0]![0]).toBe("https://custom.api/v1/address/metadata/0x5188");
  });

  it("strips trailing slash from baseUrl", async () => {
    fetchSpy.mockResolvedValueOnce(new Response(JSON.stringify(MOCK_METADATA), { status: 200 }));
    const client = createExplorerClient({ baseUrl: "https://custom.api/v1/" });
    await client.getMetadata("0x5188");
    expect(fetchSpy.mock.calls[0]![0]).toBe("https://custom.api/v1/address/metadata/0x5188");
  });
});

describe.skipIf(!process.env["TEMPO_LIVE"])("createExplorerClient (live)", () => {
  it("fetches real metadata from Moderato testnet", async () => {
    const client = createExplorerClient();
    const meta = await client.getMetadata("0x51881fed631dae3f998dad2cf0c13e0a932cbb11");
    expect(meta.address).toBe("0x51881fed631dae3f998dad2cf0c13e0a932cbb11");
    expect(meta.chainId).toBe(42431);
    expect(meta.txCount).toBeGreaterThan(0);
  });
});
