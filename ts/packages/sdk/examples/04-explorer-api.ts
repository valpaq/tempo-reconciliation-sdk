#!/usr/bin/env npx tsx
/**
 * Example: Tempo Explorer REST API client.
 * Fetches address metadata, balances, and recent transactions.
 *
 * Usage: npx tsx examples/04-explorer-api.ts
 */
import { createExplorerClient } from "../src/index";

const ADDRESS = "0x51881fed631dae3f998dad2cf0c13e0a932cbb11";

async function main() {
  const explorer = createExplorerClient();

  console.log("--- Address Metadata ---");
  const meta = await explorer.getMetadata(ADDRESS);
  console.log(`Address:     ${meta.address}`);
  console.log(`Chain ID:    ${meta.chainId}`);
  console.log(`Tx count:    ${meta.txCount}`);
  console.log(`Created:     ${new Date(meta.createdTimestamp * 1000).toISOString()}`);
  console.log(`Last active: ${new Date(meta.lastActivityTimestamp * 1000).toISOString()}`);

  console.log("\n--- Token Balances ---");
  const { balances } = await explorer.getBalances(ADDRESS);
  for (const b of balances.slice(0, 5)) {
    const amount = Number(b.balance) / 10 ** b.decimals;
    console.log(`  ${b.symbol}: ${amount.toFixed(2)} ${b.currency}`);
  }

  console.log("\n--- Recent Transactions ---");
  const history = await explorer.getHistory(ADDRESS, { limit: 5 });
  for (const tx of history.transactions) {
    const block = parseInt(tx.blockNumber, 16);
    const time = new Date(tx.timestamp * 1000).toISOString();

    const sendEvents = tx.knownEvents.filter((e) => e.type === "send");
    const memo = sendEvents.length > 0 ? sendEvents[0]!.note : null;

    console.log(`  block=${block} time=${time}`);
    console.log(`    tx=${tx.hash.slice(0, 20)}... status=${tx.status}`);
    if (memo) {
      console.log(`    memo="${memo.replace(/\0/g, "").trim()}"`);
    }
    console.log();
  }

  console.log(`Total: ${history.total} transactions, hasMore: ${history.hasMore}`);
}

main().catch(console.error);
