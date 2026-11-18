// Domain.subdomainCount: how many of a name's direct children are live.
//
// A name is live while it has an owner, a resolver, or a live child of its
// own. That is the ENSv1 subgraph's deletion convention: a name whose owner
// and resolver were both cleared, and which has no children left, reads as
// deleted and stops counting as its parent's child, and that can make the
// parent itself read as deleted, and so on up.
//
// The count only ever changes when a name crosses that line. The original
// mapping decremented the parent on every save of a row that was in the
// deleted state, so a second event for an already deleted subname (another
// NewOwner to the zero address, a Transfer, a NewResolver) took a second
// child off the parent, and it never counted a subname again when it was
// re-created, so a parent whose subname was deleted and made again read as
// having none. Counts went wrong, and negative, for any name with such a
// history. Every writer now goes through the same two steps: note whether the
// row counted before the change (countsAsChild), and after saving it pass the
// difference to the parent (addToSubdomainCount), which carries it upward
// when the parent crosses the line in turn.
//
// Used by the ENSv1 registry handlers (ensRegistry.ts), the ENSv2 projection
// and its retirement (ensv2Domain.ts, ensv2Paths.ts), and the sweeps that
// take a resolver off ENSv1 rows (legacyGovernance.ts).
import { Bytes } from "@graphprotocol/graph-ts";
import { Domain, DomainChildRows } from "./types/schema";
import { EMPTY_ADDRESS } from "./utils";

// Longest chain of parents followed when a change is carried upward. The
// ENSv1 registry is a tree of hashes with no depth limit, and each level is
// a registry transaction someone paid for; a chain this long costs about
// 250 MB of handler memory to walk, and one longer than this stops being
// followed.
const MAX_DEPTH = 100000;

// True when `domain` is counted in its parent's subdomainCount: it has a
// parent and is live. Call it on a row as it is stored, before changing it,
// and again after.
export function countsAsChild(domain: Domain): boolean {
  if (!domain.parent) {
    return false;
  }
  if (domain.resolver) {
    return true;
  }
  return domain.owner != EMPTY_ADDRESS || domain.subdomainCount > 0;
}

// A Domain row was given `parentId` as its parent, which happens once per
// row. Counts the rows, live or not, that a read of the parent's children
// would load (childRowsOf).
export function noteChildRow(parentId: string): void {
  let id = Bytes.fromHexString(parentId);
  let counter = DomainChildRows.load(id);
  if (counter == null) {
    counter = new DomainChildRows(id);
    counter.rows = 0;
  }
  counter.rows = counter.rows + 1;
  counter.save();
}

export function childRowsOf(parentId: string): i32 {
  let counter = DomainChildRows.load(Bytes.fromHexString(parentId));
  return counter == null ? 0 : counter.rows;
}

// Saves a row whose owner or resolver may just have changed and passes the
// difference on to its parent. `wasCounted` is countsAsChild(domain) as the
// row stood before the change, false for a row created now. Every write that
// can set or clear a Domain's owner or resolver ends here or does the same
// two steps itself.
export function saveCounted(domain: Domain, wasCounted: boolean): void {
  domain.save();
  let isCounted = countsAsChild(domain);
  if (wasCounted != isCounted) {
    addToSubdomainCount(domain.parent!, isCounted ? 1 : -1);
  }
}

// `delta` children of `parentId` became live (positive) or stopped being so
// (negative). Loads the parent fresh and saves it: a caller that holds the
// parent row must not save its own copy afterwards.
export function addToSubdomainCount(parentId: string, delta: i32): void {
  let id = parentId;
  let change = delta;
  for (let depth = 0; depth < MAX_DEPTH; depth++) {
    if (change == 0) {
      return;
    }
    let parent = Domain.load(id);
    if (parent == null) {
      return;
    }
    let counted = countsAsChild(parent);
    let next = parent.subdomainCount + change;
    parent.subdomainCount = next < 0 ? 0 : next;
    parent.save();
    if (counted == countsAsChild(parent)) {
      return;
    }
    // It crossed the line, so it has a parent (countsAsChild was true on one
    // side).
    change = counted ? -1 : 1;
    id = parent.parent!;
  }
}
