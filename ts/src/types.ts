export type MemoType = "invoice" | "payroll" | "refund" | "batch" | "subscription" | "custom";

export type MemoV1 = {
  v: 1;
  t: MemoType;
  issuerTag: bigint;
  ulid: string;
  id16: Uint8Array;
  salt: Uint8Array;
  raw: `0x${string}`;
};

export type EncodeMemoV1Params = {
  type: MemoType;
  issuerTag: bigint;
  ulid: string;
  salt?: Uint8Array | "random";
};

export type PaymentEvent = {
  chainId: number;
  blockNumber: bigint;
  txHash: `0x${string}`;
  logIndex: number;
  token: `0x${string}`;
  from: `0x${string}`;
  to: `0x${string}`;
  amount: bigint;
  memoRaw?: `0x${string}`;
  memo?: MemoV1 | string | null;
  timestamp?: number;
};

export type ExpectedPayment = {
  memoRaw: `0x${string}`;
  token: `0x${string}`;
  to: `0x${string}`;
  amount: bigint;
  from?: `0x${string}`;
  /** Payment deadline as unix timestamp in seconds (not milliseconds). */
  dueAt?: number;
  meta?: Record<string, string>;
};

export type MatchStatus =
  | "matched"
  | "partial"
  | "unknown_memo"
  | "no_memo"
  | "mismatch_amount"
  | "mismatch_token"
  | "mismatch_party"
  | "expired";

export type MatchResult = {
  status: MatchStatus;
  payment: PaymentEvent;
  expected?: ExpectedPayment;
  reason?: string;
  overpaidBy?: bigint;
  remainingAmount?: bigint;
  isLate?: boolean;
};

export type ReconcileSummary = {
  totalExpected: number;
  totalReceived: number;
  matchedCount: number;
  issueCount: number;
  pendingCount: number;
  totalExpectedAmount: bigint;
  totalReceivedAmount: bigint;
  totalMatchedAmount: bigint;
  unknownMemoCount: number;
  noMemoCount: number;
  mismatchAmountCount: number;
  mismatchTokenCount: number;
  mismatchPartyCount: number;
  expiredCount: number;
  partialCount: number;
};

export type ReconcileReport = {
  matched: MatchResult[];
  issues: MatchResult[];
  pending: ExpectedPayment[];
  summary: ReconcileSummary;
};

export type WatchOptions = {
  rpcUrl: string;
  chainId: number;
  token: `0x${string}`;
  to?: `0x${string}`;
  from?: `0x${string}`;
  startBlock?: bigint;
  pollIntervalMs?: number;
  dedupeTtlMs?: number;
  includeTransferOnly?: boolean;
  onError?: (err: Error) => void;
};

export type HistoryOptions = WatchOptions & {
  fromBlock: bigint;
  toBlock?: bigint;
  batchSize?: number;
};

export type WatchWsOptions = {
  wsUrl: string;
  chainId: number;
  token: `0x${string}`;
  to?: `0x${string}`;
  from?: `0x${string}`;
  includeTransferOnly?: boolean;
  dedupeTtlMs?: number;
  onError?: (err: Error) => void;
  /** Max reconnection attempts after WebSocket drops. 0 = no reconnect. Default: 5. */
  maxReconnects?: number;
  /** Base delay (ms) before first reconnect. Doubles each attempt, capped at 30s. Default: 1000. */
  reconnectDelayMs?: number;
};

export type ReconcileStore = {
  /** Store an expected payment keyed by memoRaw. May throw on duplicates. */
  addExpected(payment: ExpectedPayment): void;
  getExpected(memoRaw: `0x${string}`): ExpectedPayment | undefined;
  getAllExpected(): ExpectedPayment[];
  removeExpected(memoRaw: `0x${string}`): boolean;
  addResult(key: string, result: MatchResult): void;
  getResult(key: string): MatchResult | undefined;
  getAllResults(): MatchResult[];
  addPartial(memoRaw: `0x${string}`, amount: bigint): bigint;
  getPartialTotal(memoRaw: `0x${string}`): bigint;
  removePartial(memoRaw: `0x${string}`): void;
  clear(): void;
};

export type ReconcilerOptions = {
  store?: ReconcileStore;
  issuerTag?: bigint;
  strictSender?: boolean;
  allowOverpayment?: boolean;
  rejectExpired?: boolean;
  amountToleranceBps?: number;
  allowPartial?: boolean;
  /**
   * How tolerance interacts with partial payments.
   * - `"final"` (default): tolerance applies to the cumulative total only.
   *   Each partial is accepted as-is; match triggers when `cumulative >= expected - tolerance`.
   * - `"each"`: tolerance applies per-payment. If a single payment is underpaid
   *   beyond tolerance, it's rejected as `mismatch_amount` instead of becoming `partial`.
   */
  partialToleranceMode?: "final" | "each";
};

export type WebhookBatchError = {
  results: MatchResult[];
  statusCode?: number;
  error?: string;
};

export type WebhookOptions = {
  url: string;
  results: MatchResult[];
  secret?: string;
  batchSize?: number;
  maxRetries?: number;
  /** Request timeout in ms per batch. Default: 30000 (30s). */
  timeoutMs?: number;
  fetch?: typeof globalThis.fetch;
  onBatchError?: (err: WebhookBatchError) => void;
};

export type WebhookResult = {
  sent: number;
  failed: number;
  errors: WebhookBatchError[];
};

export type ExplorerOptions = {
  baseUrl?: string;
  fetch?: typeof globalThis.fetch;
};

export type AddressMetadata = {
  address: `0x${string}`;
  chainId: number;
  accountType: string;
  txCount: number;
  lastActivityTimestamp: number;
  createdTimestamp: number;
  createdTxHash: `0x${string}`;
  createdBy: `0x${string}`;
};

export type TokenBalance = {
  token: `0x${string}`;
  /** Raw balance as string (represents a bigint value from the Explorer API). */
  balance: string;
  name: string;
  symbol: string;
  currency: string;
  decimals: number;
};

export type BalancesResponse = {
  balances: TokenBalance[];
};

export type KnownEventPart = {
  type: "action" | "amount" | "text" | "account";
  value: string | { token: string; value: string; decimals: number; symbol: string };
};

export type KnownEvent = {
  type: string;
  note?: string;
  parts: KnownEventPart[];
  meta?: Record<string, string>;
};

export type ExplorerTransaction = {
  hash: `0x${string}`;
  blockNumber: string;
  timestamp: number;
  from: `0x${string}`;
  to: `0x${string}`;
  value: string;
  status: string;
  gasUsed: string;
  effectiveGasPrice: string;
  knownEvents: KnownEvent[];
};

export type HistoryResponse = {
  transactions: ExplorerTransaction[];
  total: number;
  offset: number;
  limit: number;
  hasMore: boolean;
  countCapped: boolean;
  error: string | null;
};
