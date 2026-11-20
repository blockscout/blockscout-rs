// Which side writes a legacy Domain row.
//
// ENSv1 handlers (ENSRegistry, BaseRegistrar, NameWrapper) and the ENSv2
// projection (ensv2Domain.ts) both write the legacy Domain / Registration /
// WrappedDomain entities. Without a precedence rule the last event wins, so
// an ENSv1 event that arrives after a name is live in ENSv2 (in practice:
// Graveyard.clear() run over the v1 record of a name that was migrated or
// registered natively in ENSv2) overwrites the live ENSv2 state with retired
// ENSv1 state. Three rules prevent that:
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
//
// 3. An ENSv1 name below an ENSv2 name takes no resolver from ENSv1.
//    Resolution walks the ENSv2 registries from the root and uses the deepest
//    resolver it finds (contracts-v2 LibResolution.findResolver). An
//    unmigrated .eth name is RESERVED in the ETHRegistry with the
//    ENSV1Resolver, which looks the whole name up in ENSv1, so it and its
//    ENSv1 subnames keep resolving as before. Once the name is REGISTERED in
//    ENSv2 (migrated, or registered again after it lapsed) it has its own
//    resolver and subregistry, and an ENSv1 name below it that ENSv2 does not
//    hold is answered by that resolver, or not at all: never by its ENSv1
//    resolver, although the ENSv1 registry still has the record. So:
//      - when an ENSv2 name is projected, every ENSv1 Domain row below it
//        loses its resolver and resolved address, at every depth
//        (voidLegacySubtree), and the name is marked for good
//        (ENSv2NamePath.legacySubtreeVoided);
//      - from then on an ENSv1 NewResolver for a name below it only writes
//        its history row (isDetachedFromEnsv1). ENSv1 resolver record events
//        need no guard: they only reach a Domain whose resolver is set.
//    The mark is permanent. A name below shows an address again only when
//    ENSv2 projects it. A name that points its ENSv2 resolver at the
//    ENSV1Resolver by hand does send its ENSv1 subnames back to ENSv1; they
//    are shown without an address all the same, the safe side.
//
//    One case does keep resolving through ENSv1. A locked NameWrapper name
//    migrates into a WrapperRegistry, whose getResolver() returns the
//    ENSV1Resolver for a child that is emancipated (PARENT_CANNOT_CONTROL
//    burned, not expired), still has an ENSv1 owner and was never registered
//    in that registry (WrapperRegistry._isMigratableChild). Such a child and
//    everything below it stay ENSv1's until the child migrates. The migrated
//    name is then left unmarked, its other ENSv1 rows are cleared all the
//    same, and the exception is evaluated from current state on every ENSv1
//    event (isServedByEnsv1). It ends, with the rows cleared at that moment:
//      - for everything below the name, when the name stops pointing at a
//        WrapperRegistry, is retired, or is registered again;
//      - for one child, when it is registered in the WrapperRegistry (rule
//        1 takes over) or leaves the NameWrapper (cutOffLegacyHoldout). On
//        chain an unwrapped child stays eligible until its wrapper expiry,
//        but the subgraph drops fuses and expiry on unwrap and could not
//        tell when that is, so it stops there.
//    A child whose wrapper expiry passes emits nothing and keeps its row,
//    like every expired name: consumers compare Domain.expiryDate with the
//    current time.
import { BigInt, Bytes, ethereum, log } from "@graphprotocol/graph-ts";
import { getEnsv2FirstBlock } from "./ensv2Constants";
import { slotEntityIdForLabelhash } from "./ensv2Utils";
import { EMPTY_ADDRESS } from "./utils";
import {
  addToSubdomainCount,
  childRowsOf,
  countsAsChild,
} from "./subdomainCount";
import {
  Domain,
  ENSv2LegacyCutoff,
  ENSv2NamePath,
  ENSv2NameSlot,
  ENSv2Registry,
  WrappedDomain,
} from "./types/schema";

// NameWrapper fuses.
const CANNOT_UNWRAP: i32 = 1;
const PARENT_CANNOT_CONTROL: i32 = 65536;

// ENSv1 rows one event's sweeps may visit, all of them together
// (voidLegacyRowsBelow). A sweep runs once per ENSv2 name, but one event can
// project thousands of names (a backfill) or retire as many, each with ENSv1
// rows below it, and a row visited costs about 7 KB of handler memory that
// is not given back. Each row is an ENSv1 name someone paid a registry
// transaction to create, so this only guards against a handler that would
// not finish. Rows past it keep their resolver; a warning is logged.
export const MAX_LEGACY_ROWS_VOIDED = 100000;

// Module state, as in ensv2Paths.ts: graph-node runs each handler in a
// fresh instance, matchstick runs every test in one, so the handlers that
// can sweep reset the budget themselves (resetLegacySweepBudget).
let maxLegacyRowsVoided: i32 = MAX_LEGACY_ROWS_VOIDED;
let legacyRowsBudget: i32 = MAX_LEGACY_ROWS_VOIDED;

export function resetLegacySweepBudget(): void {
  legacyRowsBudget = maxLegacyRowsVoided;
}

// Tests only: the real limit takes too long to reach in matchstick.
export function setMaxLegacyRowsVoidedForTesting(limit: i32): void {
  maxLegacyRowsVoided = limit;
  legacyRowsBudget = limit;
}

export function legacySweepBudgetForTesting(): i32 {
  return legacyRowsBudget;
}

// Levels isDetachedFromEnsv1 walks up. A DNS-encoded name of 255 bytes has
// at most 127 labels; a name deeper than this is treated as detached.
const MAX_LEGACY_DEPTH = 128;

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
  // RootRegistry paths (eth, reverse) have no Domain
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

// True when the ENSv1 name is locked in the NameWrapper as far as its
// WrappedDomain row says. Read at the moment a migration registers the name
// in ENSv2, before the projection drops the row: the migration itself proves
// the name had not expired (LockedWrapperReceiver reads the live fuses), so
// the stored expiry, which a .eth name's renewals do not update, is not
// consulted.
export function isLockedInEnsv1(domainId: string): boolean {
  let wrapped = WrappedDomain.load(domainId);
  if (wrapped == null) {
    return false;
  }
  return (wrapped.fuses & CANNOT_UNWRAP) != 0;
}

// The ENSv1 half of WrapperRegistry._isMigratableChild: emancipated, not
// expired, with an ENSv1 owner, and not cut off earlier.
function isEmancipatedInEnsv1(child: Domain, timestamp: BigInt): boolean {
  if (child.owner == EMPTY_ADDRESS) {
    return false;
  }
  let wrapped = WrappedDomain.load(child.id);
  if (wrapped == null) {
    return false;
  }
  if ((wrapped.fuses & PARENT_CANNOT_CONTROL) == 0) {
    return false;
  }
  if (wrapped.expiryDate.lt(timestamp)) {
    return false;
  }
  return ENSv2LegacyCutoff.load(Bytes.fromHexString(child.id)) == null;
}

// Rule 3's exception, from current state: `parentPath` is a live ENSv2 name
// that points at a WrapperRegistry, and `child` is an ENSv1 name directly
// below it that the registry has never held and still sends to ENSv1.
function isServedByEnsv1(
  parentPath: ENSv2NamePath,
  child: Domain,
  timestamp: BigInt
): boolean {
  if (!parentPath.active) {
    return false;
  }
  let slot = ENSv2NameSlot.load(parentPath.slot);
  if (slot == null || slot.status != "REGISTERED") {
    return false;
  }
  let subregistry = slot.subregistry;
  if (!subregistry) {
    return false;
  }
  let registry = ENSv2Registry.load(subregistry!);
  if (registry == null || registry.kind != "WRAPPER") {
    return false;
  }
  let labelhash = child.labelhash;
  if (!labelhash) {
    return false;
  }
  // Registered or reserved there at any time: ENSv2 is the authority.
  let slotId = slotEntityIdForLabelhash(registry.id, labelhash!);
  if (ENSv2NameSlot.load(slotId) != null) {
    return false;
  }
  return isEmancipatedInEnsv1(child, timestamp);
}

// Rule 3's guard for an ENSv1 NewResolver. Walks from the name to the root:
// a marked ENSv2 name anywhere above detaches it, and so does an unmarked
// one (a locked migration) unless the step below it on the way down is
// either an ENSv2 name itself or one the WrapperRegistry sends to ENSv1.
export function isDetachedFromEnsv1(node: Bytes, block: ethereum.Block): boolean {
  if (block.number.lt(getEnsv2FirstBlock())) {
    return false;
  }
  let loaded = Domain.load(node.toHexString());
  if (loaded == null) {
    return false;
  }
  let child: Domain = loaded;
  for (let depth = 0; depth < MAX_LEGACY_DEPTH; depth++) {
    let parentId = child.parent;
    if (!parentId) {
      return false;
    }
    let parentPath = ENSv2NamePath.load(Bytes.fromHexString(parentId!));
    if (parentPath != null) {
      if (parentPath.legacySubtreeVoided) {
        return true;
      }
      // RootRegistry paths (eth, reverse) have no projection and mark
      // nothing unless a link under them was refused.
      if (parentPath.domain) {
        let childNode = Bytes.fromHexString(child.id);
        if (
          !isGovernedByEnsv2(childNode, block.number) &&
          !isServedByEnsv1(parentPath, child, block.timestamp)
        ) {
          return true;
        }
      }
    }
    let parent = Domain.load(parentId!);
    if (parent == null) {
      return false;
    }
    child = parent!;
  }
  return true;
}

// Takes the resolver off an ENSv1 row. A row that had no owner left was live
// through its resolver alone, and stops counting as its parent's child
// (subdomainCount.ts).
function clearLegacyResolver(domain: Domain): void {
  let wasCounted = countsAsChild(domain);
  domain.resolver = null;
  domain.resolvedAddress = null;
  domain.save();
  if (wasCounted && !countsAsChild(domain)) {
    addToSubdomainCount(domain.parent!, -1);
  }
}

// Clears resolver and resolved address of every ENSv1 row below `root`, at
// every depth. A row with an ENSv2 path is skipped with its subtree: the row
// is ENSv2's (or was retired by it), and the rows below it were handled when
// that name was projected. With `keepHoldouts`, a direct child that is
// emancipated in ENSv1 is skipped with its subtree as well.
//
// One store query per row visited. Domain.subdomainCount counts a row's live
// children (subdomainCount.ts), but this does not rely on a zero there to
// skip the query: a child indexed before its parent's row existed (a reset
// testnet) is not counted, and a row left with a resolver is exactly what
// must not be missed. Returns false when it stopped at the row limit.
//
// The query loads every child row the name has ever had, deleted ones
// included, and a handler cannot give that memory back. So the limit is
// checked against the number of rows (childRowsOf) before the query runs,
// not against what it returned.
function voidLegacyRowsBelow(
  root: Domain,
  keepHoldouts: boolean,
  timestamp: BigInt
): boolean {
  let pending = new Array<string>();
  pending.push(root.id);
  while (pending.length > 0) {
    let parentId = pending.pop();
    let parent = Domain.load(parentId);
    if (parent == null) {
      continue;
    }
    if (
      legacyRowsBudget <= 0 ||
      parent.subdomainCount > legacyRowsBudget ||
      childRowsOf(parent.id) > legacyRowsBudget
    ) {
      log.warning(
        "voidLegacyRowsBelow: more ENSv1 rows than one event's sweeps visit, stopped below {} at {}",
        [root.id, parent.id]
      );
      return false;
    }
    let isRoot = parentId == root.id;
    let children = parent.subdomains.load();
    legacyRowsBudget -= children.length;
    for (let i = 0; i < children.length; i++) {
      let child = children[i];
      let childPath = ENSv2NamePath.load(Bytes.fromHexString(child.id));
      if (childPath != null) {
        if (childPath.domain) {
          continue;
        }
      }
      if (keepHoldouts && isRoot && isEmancipatedInEnsv1(child, timestamp)) {
        continue;
      }
      if (child.resolver || child.resolvedAddress) {
        clearLegacyResolver(child);
      }
      pending.push(child.id);
    }
  }
  return true;
}

// Rule 3's sweep for the ENSv2 name `path`. Runs once per name: the mark
// then keeps ENSv1 from writing a resolver below it, so there is nothing to
// clear a second time. With `keepHoldouts` (a locked migration) the name is
// left unmarked and its emancipated ENSv1 children are kept; a later call
// without it clears those too and sets the mark.
export function voidLegacySubtree(
  path: ENSv2NamePath,
  keepHoldouts: boolean,
  timestamp: BigInt
): void {
  if (path.legacySubtreeVoided) {
    return;
  }
  // A name cut off when it left the NameWrapper was swept then, and nothing
  // below it has taken a resolver since.
  let complete = true;
  if (ENSv2LegacyCutoff.load(path.id) == null) {
    let root = Domain.load(path.namehash.toHexString());
    if (root != null) {
      complete = voidLegacyRowsBelow(root, keepHoldouts, timestamp);
    }
  }
  if (keepHoldouts) {
    return;
  }
  // A projected name is marked even when the sweep stopped at the row limit:
  // isDetachedFromEnsv1 keeps ENSv1 out below it either way, and the mark
  // keeps the sweep from running again. A RootRegistry name (a whole TLD,
  // reached here only through a refused link) is marked only when all of its
  // rows were cleared, so that an oversized one is left following ENSv1
  // rather than frozen half-swept.
  if (!complete && !path.domain) {
    return;
  }
  // The caller may hold a snapshot it saves later, or none it saves at all:
  // mark both.
  path.legacySubtreeVoided = true;
  let stored = ENSv2NamePath.load(path.id);
  if (stored != null && !stored.legacySubtreeVoided) {
    stored.legacySubtreeVoided = true;
    stored.save();
  }
}

// True while `domain` is an ENSv1 name that a WrapperRegistry sends to
// ENSv1 (rule 3's exception). ENSv1 handlers read it before they change a
// name's owner or NameWrapper state, and call cutOffLegacyHoldout after.
export function isLegacyHoldout(domain: Domain, block: ethereum.Block): boolean {
  if (block.number.lt(getEnsv2FirstBlock())) {
    return false;
  }
  let parentId = domain.parent;
  if (!parentId) {
    return false;
  }
  let parentPath = ENSv2NamePath.load(Bytes.fromHexString(parentId!));
  if (parentPath == null || parentPath.legacySubtreeVoided) {
    return false;
  }
  if (!parentPath.domain) {
    return false;
  }
  return isServedByEnsv1(parentPath, domain, block.timestamp);
}

// The ENSv1 event just handled may have ended the exception for `node` (it
// left the NameWrapper, or lost its ENSv1 owner). If it did, the name and
// everything below it lose resolver and address, once and for good.
export function cutOffLegacyHoldout(node: Bytes, block: ethereum.Block): void {
  // The ENSv1 handlers call this once per event and sweep nowhere else.
  resetLegacySweepBudget();
  cutOffLegacyHoldoutWithinEvent(node, block);
}

// The same for a handler that has reset the event's budgets itself and may
// call this more than once.
export function cutOffLegacyHoldoutWithinEvent(
  node: Bytes,
  block: ethereum.Block
): void {
  let domain = Domain.load(node.toHexString());
  if (domain == null || isLegacyHoldout(domain, block)) {
    return;
  }
  if (ENSv2LegacyCutoff.load(node) != null) {
    return;
  }
  let cutoff = new ENSv2LegacyCutoff(node);
  cutoff.blockNumber = block.number;
  cutoff.timestamp = block.timestamp;
  cutoff.save();
  if (domain.resolver || domain.resolvedAddress) {
    clearLegacyResolver(domain);
  }
  voidLegacyRowsBelow(domain, false, block.timestamp);
}
