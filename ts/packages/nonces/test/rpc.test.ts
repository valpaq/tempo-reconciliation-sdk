import { describe, it, expect, vi, beforeEach } from "vitest";
import { getNonceFromPrecompile, getProtocolNonce } from "../src/rpc";
import { NONCE_PRECOMPILE, INONCE_ABI } from "../src/constants";

const mockReadContract = vi.fn();
const mockGetTransactionCount = vi.fn();
const mockClient = {
  readContract: mockReadContract,
  getTransactionCount: mockGetTransactionCount,
} as never;

beforeEach(() => {
  vi.resetAllMocks();
});

describe("getNonceFromPrecompile", () => {
  it("calls readContract with correct precompile address and ABI", async () => {
    mockReadContract.mockResolvedValue(42n);

    const result = await getNonceFromPrecompile(
      mockClient,
      "0x1234567890abcdef1234567890abcdef12345678",
      1n,
    );

    expect(mockReadContract).toHaveBeenCalledWith({
      address: NONCE_PRECOMPILE,
      abi: INONCE_ABI,
      functionName: "getNonce",
      args: ["0x1234567890abcdef1234567890abcdef12345678", 1n],
    });
    expect(result).toBe(42n);
  });

  it("returns result as bigint", async () => {
    mockReadContract.mockResolvedValue(0n);
    const result = await getNonceFromPrecompile(mockClient, "0x00", 5n);
    expect(typeof result).toBe("bigint");
    expect(result).toBe(0n);
  });

  it("passes large nonceKey values correctly", async () => {
    const maxUint256 = 2n ** 256n - 1n;
    mockReadContract.mockResolvedValue(7n);

    await getNonceFromPrecompile(mockClient, "0xabc", maxUint256);

    expect(mockReadContract).toHaveBeenCalledWith(
      expect.objectContaining({
        args: ["0xabc", maxUint256],
      }),
    );
  });

  it("propagates RPC errors", async () => {
    mockReadContract.mockRejectedValue(new Error("RPC timeout"));
    await expect(getNonceFromPrecompile(mockClient, "0x00", 1n)).rejects.toThrow("RPC timeout");
  });

  it("rejects when readContract throws ABI decode error", async () => {
    mockReadContract.mockRejectedValue(new Error("ABI decode failed"));
    await expect(getNonceFromPrecompile(mockClient, "0x00", 1n)).rejects.toThrow("ABI decode");
  });
});

describe("getProtocolNonce", () => {
  it("calls getTransactionCount with pending block tag", async () => {
    mockGetTransactionCount.mockResolvedValue(10);

    const result = await getProtocolNonce(mockClient, "0x1234567890abcdef1234567890abcdef12345678");

    expect(mockGetTransactionCount).toHaveBeenCalledWith({
      address: "0x1234567890abcdef1234567890abcdef12345678",
      blockTag: "pending",
    });
    expect(result).toBe(10n);
  });

  it("returns result as bigint", async () => {
    mockGetTransactionCount.mockResolvedValue(0);
    const result = await getProtocolNonce(mockClient, "0x00");
    expect(typeof result).toBe("bigint");
  });

  it("propagates RPC errors", async () => {
    mockGetTransactionCount.mockRejectedValue(new Error("connection refused"));
    await expect(getProtocolNonce(mockClient, "0x00")).rejects.toThrow("connection refused");
  });
});
