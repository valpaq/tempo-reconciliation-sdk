#!/usr/bin/env npx tsx
/**
 * Example: watch pathUSD transfers via WebSocket (push, not polling).
 * Uses eth_subscribe under the hood for lower latency than HTTP polling.
 * Press Ctrl+C to stop.
 *
 * Usage: npx tsx examples/05-watch-websocket.ts
 */
import { watchTip20TransfersWs, decodeMemo } from "../src/index";

const WS_URL = "wss://rpc.moderato.tempo.xyz";
const CHAIN_ID = 42431;
const PATH_USD: `0x${string}` = "0x20C0000000000000000000000000000000000000";

console.log("Watching pathUSD transfers via WebSocket...");
console.log(`WSS: ${WS_URL} | Chain: ${CHAIN_ID} | Token: ${PATH_USD}`);
console.log("Press Ctrl+C to stop.\n");

const unsubscribe = watchTip20TransfersWs(
  {
    wsUrl: WS_URL,
    chainId: CHAIN_ID,
    token: PATH_USD,
    onError: (err) => console.error("WS error:", err.message),
  },
  (event) => {
    const memoDisplay = event.memoRaw
      ? (decodeMemo(event.memoRaw) ?? event.memoRaw.slice(0, 20) + "...")
      : "(none)";

    const amount = Number(event.amount) / 1e6;

    console.log(
      `[block ${event.blockNumber}]`,
      `${event.from.slice(0, 10)}...`,
      `-> ${event.to.slice(0, 10)}...`,
      `${amount.toFixed(2)} pathUSD`,
      `memo: ${typeof memoDisplay === "string" ? memoDisplay : `v1/${memoDisplay.t}/${memoDisplay.ulid}`}`,
    );
  },
);

process.on("SIGINT", () => {
  console.log("\nStopping WebSocket watcher...");
  unsubscribe();
  process.exit(0);
});
