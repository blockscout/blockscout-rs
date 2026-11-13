// Which side writes a legacy Domain row.
//
// ENSv1 handlers (ENSRegistry, BaseRegistrar, NameWrapper) and the ENSv2
// projection (ensv2Domain.ts) both write the legacy Domain / Registration /
// WrappedDomain entities. Without a precedence rule the last event wins, so
// an ENSv1 event that arrives after a name is live in ENSv2 (in practice:
// Graveyard.clear() run over the v1 record of a name that was migrated or
// registered natively in ENSv2) overwrites the live ENSv2 state with retired
// ENSv1 state. Two rules prevent that:
//
// 1. ENSv2 governs a live ENSv2 name. While a node has an active, projected
//    ENSv2NamePath whose slot is REGISTERED, the ENSv2 projection owns the
//    node's legacy holder, expiry and resolver fields. ENSv1 registry,
//    BaseRegistrar and NameWrapper events for it only write their history
//    rows. ENSv1 *resolver* record events are unaffected: they only touch a
//    Domain whose resolver is that resolver, which is how an ENSv2 name that
//    points at a v1 PublicResolver keeps its records.
//
// 2. A sink never becomes a holder (ensv2Constants.isSink). Handing an ENSv1
//    record or token to the Graveyard, the GraveyardRepairer or a migration
//    controller only writes a history row. Migrations always void the v1
//    state first and register in ENSv2 last, in the same transaction
//    (UnlockedMigrationController, LockedWrapperReceiver), so rule 1's
//    projection then writes the new holder. Retirements (Graveyard.clear,
//    GraveyardRepairer.repair) never register in ENSv2: a retired .eth name
//    keeps its lapsed state and reads like any expired v1 name, and a retired
//    subname is deleted with the ENSv1 convention (owner = zero address).
import { BigInt, Bytes } from "@graphprotocol/graph-ts";
import { getEnsv2FirstBlock } from "./ensv2Constants";
import { ENSv2NamePath, ENSv2NameSlot } from "./types/schema";

export function isGovernedByEnsv2(node: Bytes, blockNumber: BigInt): boolean {
  // No ENSv2 path exists before ENSv2 was deployed; skip the lookup for the
  // ENSv1 history before it.
  if (blockNumber.lt(getEnsv2FirstBlock())) {
    return false;
  }
  let path = ENSv2NamePath.load(node);
  if (path == null || !path.active) {
    return false;
  }
  // RootRegistry paths (eth, reverse) and late-linked names have no Domain
  // projection; ENSv1 keeps writing those rows.
  if (!path.domain) {
    return false;
  }
  let slot = ENSv2NameSlot.load(path.slot);
  if (slot == null) {
    return false;
  }
  return slot.status == "REGISTERED";
}
