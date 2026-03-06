import type { MemoType } from "../types";

export const TYPE_CODES: Record<MemoType, number> = {
  invoice: 0x1,
  payroll: 0x2,
  refund: 0x3,
  batch: 0x4,
  subscription: 0x5,
  custom: 0xf,
};

export const CODE_TO_TYPE: Record<number, MemoType> = {
  0x1: "invoice",
  0x2: "payroll",
  0x3: "refund",
  0x4: "batch",
  0x5: "subscription",
  0xf: "custom",
};

export const MEMO_BYTES = 32;
export const ISSUER_TAG_OFFSET = 1;
export const ISSUER_TAG_SIZE = 8;
export const ID16_OFFSET = 9;
export const ID16_SIZE = 16;
export const SALT_OFFSET = 25;
export const SALT_SIZE = 7;
