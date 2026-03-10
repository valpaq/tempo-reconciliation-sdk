import { describe, it, expect } from "vitest";
import { buildAddressFilter } from "../../src/watcher/utils";

describe("buildAddressFilter", () => {
  it("returns empty object when no args provided", () => {
    expect(buildAddressFilter()).toEqual({});
  });

  it("includes only 'to' when only to is provided", () => {
    expect(buildAddressFilter("0xaaaa" as `0x${string}`)).toEqual({ to: "0xaaaa" });
  });

  it("includes only 'from' when only from is provided", () => {
    expect(buildAddressFilter(undefined, "0xbbbb" as `0x${string}`)).toEqual({ from: "0xbbbb" });
  });

  it("includes both to and from when both are provided", () => {
    expect(buildAddressFilter("0xaaaa" as `0x${string}`, "0xbbbb" as `0x${string}`)).toEqual({
      to: "0xaaaa",
      from: "0xbbbb",
    });
  });

  it("does not include keys with undefined values", () => {
    const result = buildAddressFilter(undefined, undefined);
    expect(Object.keys(result)).toHaveLength(0);
  });
});
