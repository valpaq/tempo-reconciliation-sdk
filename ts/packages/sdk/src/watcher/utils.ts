/**
 * Build the `args` filter object for getLogs / watchContractEvent.
 * Only includes `to` and `from` if they are defined.
 */
export function buildAddressFilter(
  to?: `0x${string}`,
  from?: `0x${string}`,
): { to?: `0x${string}`; from?: `0x${string}` } {
  return {
    ...(to ? { to } : {}),
    ...(from ? { from } : {}),
  };
}
