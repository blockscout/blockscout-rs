import { Address, BigInt, dataSource, ethereum, log } from "@graphprotocol/graph-ts";

import { RNSUnified } from "./types/RNSUnifiedBackfill/RNSUnified";
import { BackfillState, Domain } from "./types/schema";
import { loadDomain, loadRoot } from "./domain";
import { nodeId } from "./utils";

// Ronin became an OP Stack L2 on 2026-05-12. Its RPC serves no block before the
// L2 genesis (mainnet 55,577,500, Saigon 45,528,550), so the RecordUpdated,
// Transfer and NameChanged events of every name minted earlier are out of
// reach. The contract state was migrated, and RNSUnified is ERC721Enumerable, so
// this handler walks tokenByIndex once and loads each token with getRecord.
//
// Each token costs up to three eth_calls (tokenByIndex, getRecord, name() for
// reverse nodes) and graph-node allows about 2,000 per handler (ETHEREUM_CALL
// gas), so a block loads 400 tokens. The data source's endBlock bounds the
// window; 1,000 blocks hold 400k tokens against about 191k minted.
export const BATCH_SIZE = 400;
const STATE_ID = "rns";

export function handleBackfillBlock(block: ethereum.Block): void {
  runBackfill(block, dataSource.address(), BATCH_SIZE);
}

// Burning swaps the last token into the burned slot and shrinks the array
// (ERC721Enumerable), and mints append at the end. Walking downwards and
// clamping the cursor to the current supply means no token still below the
// cursor is skipped; tokens that move above it were already loaded, and
// tokens minted meanwhile are handled by their events.
export function runBackfill(
  block: ethereum.Block,
  registry: Address,
  batchSize: i32
): void {
  let contract = RNSUnified.bind(registry);
  let state = BackfillState.load(STATE_ID);
  if (state == null) {
    let supply = contract.try_totalSupply();
    if (supply.reverted) {
      log.warning("RNS backfill: totalSupply() reverted at block {}", [
        block.number.toString(),
      ]);
      return;
    }
    loadRoot(block.timestamp);
    state = new BackfillState(STATE_ID);
    state.total = supply.value;
    state.cursor = supply.value.minus(BigInt.fromI32(1));
    state.loaded = BigInt.zero();
    state.done = false;
  } else if (state.done) {
    return;
  } else {
    let supply = contract.try_totalSupply();
    if (!supply.reverted && state.cursor.ge(supply.value)) {
      state.cursor = supply.value.minus(BigInt.fromI32(1));
    }
  }

  let one = BigInt.fromI32(1);
  for (let i = 0; i < batchSize && state.cursor.ge(BigInt.zero()); i++) {
    let token = contract.try_tokenByIndex(state.cursor);
    if (!token.reverted && Domain.load(nodeId(token.value)) == null) {
      if (loadDomain(token.value, registry, block) != null) {
        state.loaded = state.loaded.plus(one);
      }
    }
    state.cursor = state.cursor.minus(one);
  }

  if (state.cursor.lt(BigInt.zero())) {
    state.done = true;
    log.info("RNS backfill: finished at block {} with {} of {} tokens loaded", [
      block.number.toString(),
      state.loaded.toString(),
      state.total.toString(),
    ]);
  }
  state.save();
}
