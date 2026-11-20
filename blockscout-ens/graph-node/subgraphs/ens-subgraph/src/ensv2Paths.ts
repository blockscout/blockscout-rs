// Namespace/path materialisation.
//
//   materializePathsForSlot (called from ensv2Registry.ts::handleLabelRegistered)
//   creates paths by looping over the REGISTERING slot's own registry's
//   materialised namespaces (registry.materializedNamespaces).
//
//   handleSubregistryUpdated creates namespaces by looping over the PARENT
//   slot's existing paths (bounded by parentSlot.pathCount). A namespace
//   that is new or reactivated is then backfilled (backfillNamespace): every
//   name already registered in the child registry gets its path and Domain
//   projection, and so do the registries those names link to in turn. This
//   is the subname copy flow (deploy a registry, register the names, then
//   point the parent at it), where every name is registered before the link.
//
// What keeps this bounded. Any name's subregistry can be any address, so a
// registry can be reached under any number of names, including names of its
// own. Each namespace a registry is materialised under costs one path per
// name, and each of those paths can carry another namespace, so the number
// of paths multiplies at every level unless it is capped:
//   - a registry's names are materialised under its canonical namespace and
//     at most MAX_ALIAS_NAMESPACES others (tryMaterializeNamespace);
//   - one event backfills at most maxBackfillSlotsPerEvent names, down to
//     MAX_BACKFILL_DEPTH registries below the link; what is left of a larger
//     registry is queued and projected by the registry events that follow
//     (continueQueuedBackfills);
//   - a registry is projected at most MAX_LIFETIME_MATERIALIZATIONS times
//     under a canonical namespace and as many under aliases, ever, so
//     removing and re-making a link cannot repeat the work without end;
//   - a name has at most maxNamesBelowName names projected below it
//     (ENSv2SubtreeSize), which is what bounds the other direction: one
//     event retires everything below one slot, and it has to do so in a
//     single handler.
// A namespace that is refused stays active with `materialized: false`: the
// link is recorded, its names are not projected. One that is admitted but
// not finished has `backfilled: false`.
//
// Every limit fails towards "not projected". None of them leaves a name
// reading as live after it stopped resolving: removing a link, a
// re-registration and an unregister always retire everything below, whatever
// its depth. And a link whose names are not all projected clears the
// resolver and address of the ENSv1 rows below the linked name
// (legacyGovernance.voidLegacySubtree).
import {
  Address,
  BigInt,
  ByteArray,
  Bytes,
  crypto,
  ethereum,
  log,
  store,
} from "@graphprotocol/graph-ts";

import {
  checkValidLabel,
  concat,
  createEventID,
  ETH_NODE,
  i32ToBytes,
} from "./utils";
import {
  appendRegistryNamespaceIndex,
  isZeroAddress,
  nameSlotId,
  namespaceId,
  namespaceLinkId,
  namespacePathIndexId,
  pathNamehash,
  pathNamespaceIndexId,
  registrySlotIndexId,
  slotEntityIdForLabelhash,
  slotPathIndexId,
  toSlotId,
} from "./ensv2Utils";
import { resetLegacySweepBudget, voidLegacySubtree } from "./legacyGovernance";
import { getOrCreateRegistry } from "./ensv2Discovery";
import { getRootRegistryAddress } from "./ensv2Constants";
import {
  attachDomainResolver,
  clearDomainResolver,
  getOrCreateResolver,
} from "./ensv2ResolverProjection";
import { projectPathToDomain, retireProjectedDomain } from "./ensv2Domain";
import { addToSubdomainCount } from "./subdomainCount";
import {
  Domain,
  ENSv2BackfillQueue,
  ENSv2BackfillTask,
  ENSv2NamePath,
  ENSv2Namespace,
  ENSv2NamespaceLink,
  ENSv2NamespacePathIndex,
  ENSv2NameSlot,
  ENSv2PathNamespaceIndex,
  ENSv2Registry,
  ENSv2RegistrySlotIndex,
  ENSv2ResolverUpdate,
  ENSv2Resource,
  ENSv2SlotPathIndex,
  ENSv2SubregistryUpdate,
  ENSv2SubtreeSize,
  ENSv2Token,
} from "./types/schema";
import {
  ParentUpdated,
  ResolverUpdated,
  SubregistryUpdated,
} from "./types/RootRegistry/PermissionedRegistry";


// One event projects names at most this many registries below the link it
// changes; a namespace further down is left unmaterialised. Guards against
// registries that (directly or indirectly) link to themselves, whose
// namehashes would otherwise grow without end. It limits projection only:
// retiring names has no depth limit (drainDeactivation).
const MAX_BACKFILL_DEPTH = 16;

// A registry's names are materialised under its canonical namespace (the
// name it declares as its parent, see isCanonicalNamespace) and under at
// most this many other namespaces at a time. Anyone can point a name at
// anyone's registry, so aliases are first come, first served; the canonical
// namespace does not compete with them.
export const MAX_ALIAS_NAMESPACES = 8;

// Hard ceiling on registry.materializedNamespaces, canonical ones included.
// A registry normally has one canonical namespace. Namespaces that were
// canonical when they were linked can pile up if the registries above keep
// changing their declared parent; this stops them at a fixed number, and
// with it the number of paths a single registration writes.
export const MAX_MATERIALIZED_NAMESPACES = 16;

// Names one event may project while backfilling late-linked registries, all
// levels together. When it runs out, the namespace being backfilled keeps
// its position (backfillCursor) and joins the queue; later registry events
// take it up again with a budget of their own (continueQueuedBackfills). The
// RootRegistry and ETHRegistry are never backfilled and do not draw on it.
//
// A unit is what a name a few levels deep costs, about 50 KB of handler
// memory. Counting a name's ancestors is one load and one save per level,
// so a registry slot looked at is charged one unit plus one for every 16
// levels above it, and a subregistry found on a slot one more for every 8
// (backfillSlotCost, backfillLinkCost): a backfill deep in the tree projects
// fewer names per event instead of more memory per name. Measured in
// matchstick, a name with a resolver and a subregistry costs 120 KB two
// levels deep and 2 MB at 120, and a full budget stays near 1.5 GiB at any
// depth.
export const MAX_BACKFILL_SLOTS_PER_EVENT = 10000;

// How much may be projected below one name, at every depth, counted per slot
// over all of the slot's paths (ENSv2SubtreeSize): one unit per active name
// and NAMESPACE_UNITS per subregistry projected under a name. A link
// removed, an unregistration and a re-registration each retire everything
// below one slot, in the handler of that one event, and a handler cannot
// give memory back, so an unbounded subtree would end in a handler that can
// never complete, which stops the subgraph for good.
//
// Measured in matchstick, of the 4 GiB a handler has: retiring a name costs
// about 11 KB (13 KB with a name of 255 bytes; 25 KB before the retire loop
// was slimmed down), and a name with a resolver and a subregistry 32 KB two
// levels deep, 50 KB at the name-length limit, for three units. At 100,000
// units a full retirement is about 1.3 GiB of plain names and at most
// 1.7 GiB of names that each have a subregistry. A name or subregistry that
// would take any slot above it past the limit is not projected
// (materializePathInNamespace returns null, tryMaterializeNamespace false).
// The names directly under "eth" are exempt (nothing is counted on "eth"):
// only ENS governance can change that link. Every other TLD is limited like
// any name.
export const MAX_NAMES_BELOW_NAME = 100000;

// Names one ParentUpdated looks at when it carries a change of canonical
// status down to the registries below (reconcileCanonicalBelow).
export const MAX_CANONICAL_RECONCILE_PATHS = 100000;

// What a projected subregistry (a materialised namespace) counts for below
// its parent name: retiring one costs about as much as two names.
export const NAMESPACE_UNITS = 2;

// Queue entries looked at per event. Entries whose namespace was unlinked or
// finished in the meantime cost a couple of loads each.
const MAX_BACKFILL_TASKS_PER_EVENT = 64;

// A name is projected only when it is at most this many bytes long, the
// longest the DNS wire format carries, and fewer than MAX_NAME_DEPTH labels
// deep (fitsNameLimits). Every name below a name repeats it in its
// ENSv2NamePath and Domain, so one very long label, which costs its
// registrant next to nothing, would multiply the memory it takes to retire
// or backfill everything below it; and the walks to the root that count a
// name on its ancestors stop at MAX_NAME_DEPTH, so a name further down
// would not be counted on all of them. TLDs and names directly under them
// (label.eth) are exempt from the length limit: nothing but their own row
// carries the label unless a name below is projected, and those are held to
// it.
export const MAX_NAME_BYTES = 255;
export const MAX_NAME_DEPTH = 128;

// How many times a registry's names may be projected under a canonical
// namespace, and how many times under an alias, over its whole life
// (ENSv2Registry.canonicalMaterializations / aliasMaterializations). Every
// projection of a namespace walks the registry's slots, and removing a link
// and making it again repeats that walk for every registry below it, for the
// price of two events. With a lifetime allowance, the work a registry can
// cause is a fixed multiple of the names registered in it. The pools are
// separate so that aliases, which anyone can create, cannot use up the
// allowance of the registry's own name.
export const MAX_LIFETIME_MATERIALIZATIONS = 32;

// Module state: graph-node runs each handler in a fresh instance, matchstick
// runs every test in one, so the handlers that can backfill reset the budget
// themselves (resetBackfillBudget).
let maxBackfillSlotsPerEvent: i32 = MAX_BACKFILL_SLOTS_PER_EVENT;
let backfillSlotBudget: i32 = MAX_BACKFILL_SLOTS_PER_EVENT;
let maxNamesBelowName: i32 = MAX_NAMES_BELOW_NAME;
let maxCanonicalReconcilePaths: i32 = MAX_CANONICAL_RECONCILE_PATHS;

// Start of a registry event: its backfill budget and its ENSv1 sweep budget.
export function resetBackfillBudget(): void {
  backfillSlotBudget = maxBackfillSlotsPerEvent;
  resetLegacySweepBudget();
}

// Tests only: the real limits take too long to reach in matchstick.
export function setMaxBackfillSlotsPerEventForTesting(limit: i32): void {
  maxBackfillSlotsPerEvent = limit;
}

export function setMaxNamesBelowNameForTesting(limit: i32): void {
  maxNamesBelowName = limit;
}

export function setMaxCanonicalReconcilePathsForTesting(limit: i32): void {
  maxCanonicalReconcilePaths = limit;
}

// ---------------------------------------------------------------------------
// Names below a name (ENSv2SubtreeSize)
// ---------------------------------------------------------------------------
//
// P(path) is the number of units strictly below a path: one for each active
// path, and NAMESPACE_UNITS for each namespace that is materialised under a
// path at or below it (a name's projected subregistry). S(slot) is the sum of P over
// the slot's paths. A path that becomes active, and a namespace that is
// materialised, adds its units to P of every path above it (the namespace's
// own parent path included) and to S of each of their slots; "eth" is left
// out. Retiring is the reverse, done per subtree: each
// retired path gives its own P back to its slot, and the paths above the
// retired subtree lose the number of units retired.

function addToSubtreeSize(id: Bytes, delta: i32): void {
  let row = ENSv2SubtreeSize.load(id);
  if (row == null) {
    if (delta <= 0) {
      return;
    }
    row = new ENSv2SubtreeSize(id);
    row.paths = 0;
  }
  let next = row.paths + delta;
  row.paths = next < 0 ? 0 : next;
  row.save();
}

// The paths above the names of a namespace, nearest first, with their slots,
// up to the TLD; "eth" is left out. Also what a name of the
// namespace starts from: its depth, and the bytes its parent's name adds to
// it.
class AncestorChain {
  pathIds: Array<Bytes> = new Array<Bytes>();
  slotIds: Array<Bytes> = new Array<Bytes>();
  // Depth of a name directly below the start path: 0 for a TLD.
  depth: i32 = 0;
  // "." and the start path's name, in UTF-8 bytes; 0 for a TLD.
  parentNameBytes: i32 = 0;
}

function ancestorsFrom(startPathId: Bytes | null): AncestorChain {
  let chain = new AncestorChain();
  let nextId = startPathId;
  for (let i = 0; i < MAX_NAME_DEPTH; i++) {
    if (!nextId) {
      break;
    }
    let path = ENSv2NamePath.load(nextId);
    if (path == null) {
      break;
    }
    if (i == 0) {
      chain.depth = path.depth + 1;
      let parentName = path.name;
      if (parentName !== null) {
        chain.parentNameBytes = String.UTF8.byteLength(parentName as string) + 1;
      }
    }
    // "eth" itself is not counted: every .eth registration would rewrite its
    // row, and only ENS governance can change that link. Any other TLD is
    // counted and limited like every name, so that what is below it can be
    // retired in one handler whoever comes to hold it.
    if (!path.parent && path.namehash.equals(ETH_NODE)) {
      break;
    }
    chain.pathIds.push(path.id);
    chain.slotIds.push(path.slot);
    nextId = path.parent;
  }
  return chain;
}

// Whether the slot's name under the chain's namespace may be projected:
// MAX_NAME_DEPTH and MAX_NAME_BYTES. The label is the one materializeNamePath
// writes.
function fitsNameLimits(chain: AncestorChain, slot: ENSv2NameSlot): boolean {
  if (chain.depth >= MAX_NAME_DEPTH) {
    return false;
  }
  if (chain.depth < 2) {
    return true;
  }
  // "[" + 64 hex digits + "]" for a label that is not shown as it is.
  let labelBytes = 66;
  if (checkValidLabel(slot.label)) {
    labelBytes = String.UTF8.byteLength(slot.label as string);
  }
  return labelBytes + chain.parentNameBytes <= MAX_NAME_BYTES;
}

// What looking at one registry slot, and following one subregistry found on
// it, takes from the event's backfill budget (MAX_BACKFILL_SLOTS_PER_EVENT).
function backfillSlotCost(chain: AncestorChain): i32 {
  return 1 + chain.pathIds.length / 16;
}

function backfillLinkCost(chain: AncestorChain): i32 {
  return (chain.pathIds.length + 4) / 8;
}

// Counts `units` more below every ancestor (one for a path that becomes
// active, NAMESPACE_UNITS for a namespace), unless that would take one of
// their slots past the limit: then nothing is counted and the path or
// namespace must not be projected. A slot is above it once for each of its
// own paths in the chain (more than once when a registry is linked below one
// of its own names) and is counted that many times.
function reserveBelowAncestors(chain: AncestorChain, units: i32): boolean {
  for (let i = 0; i < chain.slotIds.length; i++) {
    let times = 0;
    for (let j = 0; j < chain.slotIds.length; j++) {
      if (chain.slotIds[j].equals(chain.slotIds[i])) {
        times++;
      }
    }
    let row = ENSv2SubtreeSize.load(chain.slotIds[i]);
    let counted = row == null ? 0 : row.paths;
    if (counted + times * units > maxNamesBelowName) {
      return false;
    }
  }
  for (let i = 0; i < chain.pathIds.length; i++) {
    addToSubtreeSize(chain.pathIds[i], units);
    addToSubtreeSize(chain.slotIds[i], units);
  }
  return true;
}

// `count` paths below `startPathId` (itself included in the walk) were
// retired.
function releaseFromAncestors(startPathId: Bytes | null, count: i32): void {
  if (count <= 0) {
    return;
  }
  let chain = ancestorsFrom(startPathId);
  for (let i = 0; i < chain.pathIds.length; i++) {
    addToSubtreeSize(chain.pathIds[i], -count);
    addToSubtreeSize(chain.slotIds[i], -count);
  }
}

// `path` was retired, and with it every path below it: its slot no longer
// carries them.
function releaseOwnSubtree(path: ENSv2NamePath): void {
  let row = ENSv2SubtreeSize.load(path.id);
  if (row == null || row.paths <= 0) {
    return;
  }
  addToSubtreeSize(path.slot, -row.paths);
  row.paths = 0;
  row.save();
}

// The list of the paths bound to a slot (ENSv2SlotPathIndex, positions 0 to
// slot.pathCount - 1; each path knows its own position, path.slotIndex).
// Every handler for a slot walks it, so it holds the slot's paths and
// nothing else: a path is added when it is created for the slot or rebound
// to it, and removed when another slot takes it over. Were rebinding only
// to append, two slots handing one path back and forth would lengthen both
// lists with every event.
//
// Like the append helpers below, these allocate from a fresh load (see
// appendRegistryNamespaceIndex in ensv2Utils.ts) and sync the caller's
// snapshot. addToSlotPaths sets path.slotIndex on the caller's object, which
// the caller saves.
function addToSlotPaths(slot: ENSv2NameSlot, path: ENSv2NamePath): void {
  let current = ENSv2NameSlot.load(slot.id);
  if (current == null) {
    current = slot;
  }
  let position = current.pathCount;
  let id = slotPathIndexId(current.id, position);
  // The row may be left over from an earlier, longer list.
  let entry = ENSv2SlotPathIndex.load(id);
  if (entry == null) {
    entry = new ENSv2SlotPathIndex(id);
    entry.slot = current.id;
    entry.index = position;
  }
  entry.path = path.id;
  entry.save();

  current.pathCount = position + 1;
  current.save();
  slot.pathCount = position + 1;
  path.slotIndex = position;
}

// Takes `path` out of the list of the slot it is bound to: the last path
// moves into its position. Call it before path.slot changes.
function removeFromSlotPaths(path: ENSv2NamePath): void {
  let position = path.slotIndex;
  path.slotIndex = -1;
  if (position < 0) {
    return;
  }
  let slot = ENSv2NameSlot.load(path.slot);
  if (slot == null) {
    return;
  }
  let last = slot.pathCount - 1;
  if (position > last) {
    return;
  }
  let entry = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, position));
  if (entry == null || !entry.path.equals(path.id)) {
    return;
  }
  if (position != last) {
    let lastEntry = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, last));
    if (lastEntry != null) {
      entry.path = lastEntry.path;
      entry.save();
      let moved = ENSv2NamePath.load(lastEntry.path);
      if (moved != null) {
        moved.slotIndex = position;
        moved.save();
      }
    }
  }
  slot.pathCount = last;
  slot.save();
}

// A registration silently resets its slot resolver even when no
// ResolverUpdated(..., address(0)) event is emitted. Clear every previously
// materialised compatibility Domain before any later same-transaction event
// can attach a new resolver.
export function clearProjectedDomainResolversForSlot(
  slot: ENSv2NameSlot,
): void {
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    // Skip paths another registry's slot has since taken over (see
    // materializePathsForSlot).
    if (path != null && path.domain != null && path.slot.equals(slot.id)) {
      clearDomainResolver(path.namehash);
    }
  }
}

function appendPathNamespaceIndex(
  path: ENSv2NamePath,
  namespace: ENSv2Namespace
): void {
  let current = ENSv2NamePath.load(path.id);
  if (current == null) {
    current = path;
  }
  let next = current.namespaceCount;
  let index = new ENSv2PathNamespaceIndex(pathNamespaceIndexId(current.id, next));
  index.path = current.id;
  index.index = next;
  index.namespace = namespace.id;
  index.save();

  current.namespaceCount = next + 1;
  current.save();
  path.namespaceCount = next + 1;
}

// The list of a namespace's active paths (ENSv2NamespacePathIndex, positions
// 0 to namespace.pathCount - 1; each path knows its own position,
// path.namespaceIndex). Retiring a namespace walks this list, and it has to
// do so in one handler, so the list holds what is live and nothing else: a
// path is added when it becomes active and removed when it is retired on its
// own. An append-only index would grow with every name the namespace ever
// had, and names registered and unregistered in turn would make it as long
// as anyone cared to pay for without ever exceeding the limit on live names.
//
// Adds `path` at the end. Sets path.namespaceIndex on the caller's object,
// which the caller saves.
function addToNamespacePaths(
  namespace: ENSv2Namespace,
  path: ENSv2NamePath
): void {
  let current = ENSv2Namespace.load(namespace.id);
  if (current == null) {
    current = namespace;
  }
  let position = current.pathCount;
  let id = namespacePathIndexId(current.id, position);
  // The row may be left over from an earlier, longer list.
  let entry = ENSv2NamespacePathIndex.load(id);
  if (entry == null) {
    entry = new ENSv2NamespacePathIndex(id);
    entry.namespace = current.id;
    entry.index = position;
  }
  entry.path = path.id;
  entry.save();

  current.pathCount = position + 1;
  current.save();
  namespace.pathCount = position + 1;
  path.namespaceIndex = position;
}

// Takes `path` out of its namespace's list: the last path moves into its
// position. Sets path.namespaceIndex on the caller's object, which the
// caller saves. Call it before path.namespace changes.
export function removeFromNamespacePaths(path: ENSv2NamePath): void {
  let position = path.namespaceIndex;
  path.namespaceIndex = -1;
  if (position < 0) {
    return;
  }
  let namespace = ENSv2Namespace.load(path.namespace);
  if (namespace == null) {
    return;
  }
  let last = namespace.pathCount - 1;
  // Not in the list any more: the namespace was retired as a whole, which
  // empties it.
  if (position > last) {
    return;
  }
  let entry = ENSv2NamespacePathIndex.load(
    namespacePathIndexId(namespace.id, position)
  );
  if (entry == null || !entry.path.equals(path.id)) {
    return;
  }
  if (position != last) {
    let lastEntry = ENSv2NamespacePathIndex.load(
      namespacePathIndexId(namespace.id, last)
    );
    if (lastEntry != null) {
      entry.path = lastEntry.path;
      entry.save();
      let moved = ENSv2NamePath.load(lastEntry.path);
      if (moved != null) {
        moved.namespaceIndex = position;
        moved.save();
      }
    }
  }
  namespace.pathCount = last;
  namespace.save();
}

// Idempotent: sets active = true whether creating or reactivating. Caller
// checks pre-existence (ENSv2Namespace.load(id) == null, before calling
// this) to decide whether to append indices — reactivation must never
// re-append. Whether the namespace is materialised is decided afterwards
// (tryMaterializeNamespace).
function createOrReactivateNamespace(
  childRegistry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  parentTokenId: BigInt,
  event: ethereum.Event
): ENSv2Namespace {
  let id = namespaceId(childRegistry.id, parentPath.namehash);
  let namespace = ENSv2Namespace.load(id);
  if (namespace == null) {
    namespace = new ENSv2Namespace(id);
    namespace.registry = childRegistry.id;
    namespace.pathCount = 0;
    namespace.materialized = false;
    namespace.backfilled = false;
    namespace.backfillCursor = 0;
    namespace.backfillQueued = false;
    namespace.canonical = false;
    namespace.createdAt = event.block.timestamp;
    namespace.createdAtBlock = event.block.number;
  }
  namespace.parentPath = parentPath.id;
  namespace.parentSlot = parentSlot.id;
  namespace.parentRegistry = parentSlot.registry;
  namespace.parentSlotId = parentSlot.slotId;
  namespace.parentTokenId = parentTokenId;
  let parentResourceId = parentSlot.currentResource;
  if (parentResourceId) {
    let resourceEntity = ENSv2Resource.load(parentResourceId);
    if (resourceEntity != null) {
      namespace.parentResource = resourceEntity.resource;
    }
  }
  namespace.baseName = parentPath.name;
  namespace.baseNamehash = parentPath.namehash;
  namespace.active = true;
  namespace.updatedAtBlock = event.block.number;
  namespace.transactionID = event.transaction.hash;
  namespace.logIndex = event.logIndex;
  namespace.save();
  return namespace;
}

function upsertNamespaceLink(
  parentRegistryId: Bytes,
  parentSlot: ENSv2NameSlot,
  previousChildAddress: Bytes | null,
  event: SubregistryUpdated
): void {
  // A slot can only point at one subregistry at a time. Deactivate the
  // superseded link and narrow nullable Bytes before calling .equals().
  if (previousChildAddress) {
    let newChildAddress: Bytes = isZeroAddress(event.params.subregistry)
      ? Bytes.empty()
      : event.params.subregistry;
    let isSameTarget = previousChildAddress.equals(newChildAddress);
    if (!isSameTarget) {
      let oldLinkId = namespaceLinkId(
        parentRegistryId,
        parentSlot.slotId,
        previousChildAddress
      );
      let oldLink = ENSv2NamespaceLink.load(oldLinkId);
      if (oldLink != null) {
        oldLink.active = false;
        oldLink.save();
      }
    }
  }

  if (isZeroAddress(event.params.subregistry)) {
    return;
  }

  let childAddress = event.params.subregistry;
  let linkId = namespaceLinkId(parentRegistryId, parentSlot.slotId, childAddress);
  let link = ENSv2NamespaceLink.load(linkId);
  if (link == null) {
    link = new ENSv2NamespaceLink(linkId);
    link.parentRegistry = parentRegistryId;
    link.parentSlot = parentSlot.id;
    link.parentSlotId = parentSlot.slotId;
    link.childRegistryAddress = event.params.subregistry;
    link.childRegistry = childAddress;
  }
  link.parentTokenId = event.params.tokenId;
  // Keep the link's parent resource aligned with its namespace metadata.
  let parentResourceId = parentSlot.currentResource;
  if (parentResourceId) {
    let resourceEntity = ENSv2Resource.load(parentResourceId);
    if (resourceEntity != null) {
      link.parentResource = resourceEntity.resource;
    }
  }
  link.active = true;
  link.transactionID = event.transaction.hash;
  link.blockNumber = event.block.number;
  link.logIndex = event.logIndex;
  link.save();
}

// Marks a namespace inactive and gives up its place among its registry's
// materialised namespaces. Returns whether it was materialised, i.e. whether
// it can have live paths to deactivate.
function deactivateNamespace(
  namespace: ENSv2Namespace,
  block: ethereum.Block
): boolean {
  let wasMaterialized = namespace.materialized;
  namespace.active = false;
  namespace.materialized = false;
  namespace.updatedAtBlock = block.number;
  namespace.save();
  // Fresh load of the parent path, as for the registry below.
  let parentPathId = namespace.parentPath;
  if (parentPathId) {
    let parentPath = ENSv2NamePath.load(parentPathId);
    if (parentPath != null) {
      let pointer = parentPath.activeNamespace;
      if (pointer && pointer.equals(namespace.id)) {
        parentPath.activeNamespace = null;
        parentPath.save();
      }
    }
  }
  if (wasMaterialized) {
    // Fresh load: callers further up may hold older snapshots.
    let registry = ENSv2Registry.load(namespace.registry);
    if (registry != null) {
      let current = registry.materializedNamespaces;
      let remaining = new Array<Bytes>();
      for (let i = 0; i < current.length; i++) {
        if (!current[i].equals(namespace.id)) {
          remaining.push(current[i]);
        }
      }
      registry.materializedNamespaces = remaining;
      registry.save();
    }
  }
  return wasMaterialized;
}

// Reconstructs the same namespace ids the creation loop would have produced
// (childRegistry + each existing path's namehash) and deactivates them —
// never deletes.
export function deactivateNamespacesFromParentSlot(
  parentSlot: ENSv2NameSlot,
  previousChildRegistryId: Bytes,
  block: ethereum.Block
): void {
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let parentPath = ENSv2NamePath.load(pathIndex.path);
    if (parentPath == null || !parentPath.slot.equals(parentSlot.id)) {
      continue;
    }
    let namespace = ENSv2Namespace.load(
      namespaceId(previousChildRegistryId, parentPath.namehash)
    );
    if (namespace == null) {
      continue;
    }
    if (deactivateNamespace(namespace, block)) {
      deactivatePathsForNamespace(namespace, block);
    }
  }
}

// The slot no longer points at `previousChildRegistryId` and no
// SubregistryUpdated said so: a (re-)registration, or an unregister on an
// implementation that clears the entry. Retires the names that hung off the
// slot through it and the link itself.
export function unlinkSubregistryFromSlot(
  slot: ENSv2NameSlot,
  previousChildRegistryId: Bytes,
  block: ethereum.Block
): void {
  deactivateNamespacesFromParentSlot(slot, previousChildRegistryId, block);
  let link = ENSv2NamespaceLink.load(
    namespaceLinkId(slot.registry, slot.slotId, previousChildRegistryId)
  );
  if (link != null && link.active) {
    link.active = false;
    link.save();
  }
}

// Deactivates every path under `namespace`, which the caller has already
// marked inactive, and everything below those paths, at every depth. A name
// below a link that is gone must never keep reading as live: its Domain would
// go on showing a resolver and an address the name no longer has. The name
// the namespace hung off stays, with that many fewer names below it.
function deactivatePathsForNamespace(
  namespace: ENSv2Namespace,
  block: ethereum.Block
): void {
  let pending = new Array<Bytes>();
  pending.push(namespace.id);
  let retired = drainDeactivation(pending, block);
  releaseFromAncestors(namespace.parentPath, retired);
}

// `path` has just been retired by the caller (an unregistration). A path
// that stops resolving takes every name below it along: getSubregistry()
// returns zero for an expired or unregistered name. Deactivates the
// namespace linked below the path and all its paths, at every depth.
// Re-linking (handleSubregistryUpdated) reactivates and backfills them.
export function deactivateDescendantsOfPath(
  path: ENSv2NamePath,
  block: ethereum.Block
): void {
  let pending = new Array<Bytes>();
  collectNamespacesBelowPath(path, block, pending);
  let retired = drainDeactivation(pending, block);
  releaseOwnSubtree(path);
  releaseFromAncestors(path.parent, retired + 1);
}

// Marks the namespace linked below `path` inactive and queues it if it was
// materialised, i.e. if it can have live paths. A name has one subregistry
// at a time, so there is one namespace to look at (path.activeNamespace,
// kept by setActiveNamespace), however many the name has had: the cost of
// retiring a name does not grow with its history.
function collectNamespacesBelowPath(
  path: ENSv2NamePath,
  block: ethereum.Block,
  pending: Array<Bytes>
): void {
  let current = ENSv2NamePath.load(path.id);
  if (current == null) {
    return;
  }
  collectNamespace(current.activeNamespace, block, pending);
}

function collectNamespace(
  activeId: Bytes | null,
  block: ethereum.Block,
  pending: Array<Bytes>
): void {
  if (!activeId) {
    return;
  }
  let namespace = ENSv2Namespace.load(activeId);
  if (namespace == null || !namespace.active) {
    return;
  }
  if (deactivateNamespace(namespace, block)) {
    pending.push(namespace.id);
  }
}

// Records `namespace` as the one linked below its parent path. If another
// namespace is still active there it is retired first, with everything
// below it: this is what guarantees the single pointer is enough, rather
// than relying on every other handler having unlinked the old one.
function setActiveNamespace(
  parentPath: ENSv2NamePath,
  namespace: ENSv2Namespace,
  block: ethereum.Block
): void {
  let current = ENSv2NamePath.load(parentPath.id);
  if (current == null) {
    return;
  }
  let previousId = current.activeNamespace;
  if (previousId && previousId.equals(namespace.id)) {
    return;
  }
  if (previousId) {
    let previous = ENSv2Namespace.load(previousId);
    if (previous != null && previous.active) {
      if (deactivateNamespace(previous, block)) {
        deactivatePathsForNamespace(previous, block);
      }
      // deactivateNamespace saved the path; take its current state.
      current = ENSv2NamePath.load(parentPath.id)!;
    }
  }
  current.activeNamespace = namespace.id;
  current.save();
  parentPath.activeNamespace = namespace.id;
}

// Works through a list of namespaces instead of recursing: the tree below a
// link has no depth limit here, and a deep one must not be able to exhaust
// the wasm stack. It ends because every namespace is queued at most once
// (only on its active -> inactive transition). The work is bounded by what
// is live: a namespace lists its active paths (ENSv2NamespacePathIndex), so
// this is neither a scan over a registry's slots nor over the names a
// namespace has had, and what is live below one slot is bounded by
// maxNamesBelowName. Returns the number of units retired: the paths, and the
// namespaces they were under.
//
// This loop is why that bound exists, so it loads as little as it can per
// name: the index row, the path and its Domain. The parent's subdomainCount
// is written once per run of siblings instead of once per name.
function drainDeactivation(pending: Array<Bytes>, block: ethereum.Block): i32 {
  let retired = 0;
  while (pending.length > 0) {
    let namespace = ENSv2Namespace.load(pending.pop());
    if (namespace == null) {
      continue;
    }
    // Only materialised namespaces are queued, and each counted below its
    // parent name.
    retired += NAMESPACE_UNITS;
    // Siblings share a parent Domain; "" while none has been seen.
    let parentDomainId = "";
    let parentDecrement = 0;
    for (let i = 0; i < namespace.pathCount; i++) {
      let pathIndex = ENSv2NamespacePathIndex.load(
        namespacePathIndexId(namespace.id, i)
      );
      if (pathIndex == null) {
        continue;
      }
      let path = ENSv2NamePath.load(pathIndex.path);
      // A path another registry's namespace has since taken over is no longer
      // this namespace's to deactivate, and one that is already inactive was
      // retired when it became so.
      if (
        path == null ||
        !path.active ||
        !path.namespace.equals(namespace.id)
      ) {
        continue;
      }
      retired++;
      releaseOwnSubtree(path);
      let linkedBelow = path.activeNamespace;
      path.active = false;
      path.namespaceIndex = -1;
      path.updatedAt = block.timestamp;
      path.updatedAtBlock = block.number;
      path.save();
      // The name is no longer reachable through ENSv2: its compatibility row
      // must stop reading as live. A row that stops counting as its parent's
      // child is taken off the parent with its siblings; one that still has
      // children of its own stops counting when they go, further down this
      // list.
      let parentId = retireProjectedDomain(path, block.timestamp);
      if (parentId) {
        let id: string = parentId!;
        if (parentDecrement > 0 && parentDomainId != id) {
          addToSubdomainCount(parentDomainId, -parentDecrement);
          parentDecrement = 0;
        }
        parentDomainId = id;
        parentDecrement++;
      }
      collectNamespace(linkedBelow, block, pending);
    }
    if (parentDecrement > 0) {
      addToSubdomainCount(parentDomainId, -parentDecrement);
    }
    // Every path the namespace listed is retired: the list is empty.
    if (namespace.pathCount != 0) {
      let emptied = ENSv2Namespace.load(namespace.id);
      if (emptied != null) {
        emptied.pathCount = 0;
        emptied.save();
      }
    }
  }
  // A large retirement and a large backfill do not share one handler: what
  // was retired is taken off the event's backfill budget (a unit retired is
  // about an eighth of a name backfilled), and a backfill that no longer
  // fits waits in the queue for the next event.
  backfillSlotBudget -= retired / 8;
  return retired;
}

export function handleSubregistryUpdated(event: SubregistryUpdated): void {
  resetBackfillBudget();
  let parentRegistryId = event.address;
  let parentSlotId = toSlotId(event.params.tokenId);
  let parentSlot = ENSv2NameSlot.load(nameSlotId(parentRegistryId, parentSlotId));
  if (parentSlot == null) {
    log.warning("SubregistryUpdated for unknown slot {} on registry {}", [
      parentSlotId.toString(),
      parentRegistryId.toHexString(),
    ]);
    return;
  }

  let previousChildAddress = parentSlot.subregistry;

  parentSlot.subregistryAddress = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  parentSlot.subregistry = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  parentSlot.updatedAt = event.block.timestamp;
  parentSlot.updatedAtBlock = event.block.number;
  parentSlot.save();

  let history = new ENSv2SubregistryUpdate(createEventID(event));
  history.slot = parentSlot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.subregistryAddress = isZeroAddress(event.params.subregistry)
    ? null
    : event.params.subregistry;
  history.sender = event.params.sender;
  history.save();

  upsertNamespaceLink(parentRegistryId, parentSlot, previousChildAddress, event);

  voidHoldoutsUnlessWrapperRegistry(parentSlot, previousChildAddress, event);

  if (isZeroAddress(event.params.subregistry)) {
    if (previousChildAddress) {
      deactivateNamespacesFromParentSlot(
        parentSlot,
        previousChildAddress,
        event.block
      );
    }
    return;
  }

  // Direct non-zero-to-non-zero swap (registry A -> registry B, no
  // intervening clear-to-zero) — setSubregistry has no on-chain requirement
  // to pass through zero first, so this is reachable, not hypothetical.
  // Without this, namespaces from the superseded registry A stay
  // active:true forever and can resurface if A later gets its own
  // registrations.
  if (previousChildAddress && !previousChildAddress.equals(event.params.subregistry)) {
    deactivateNamespacesFromParentSlot(parentSlot, previousChildAddress, event.block);
  }

  // A row only: the registry's own events are indexed once it emits one
  // (ensv2Discovery.ts).
  let childRegistry = getOrCreateRegistry(
    event.params.subregistry,
    event.params.subregistry,
    event.block
  );

  // One namespace per active path of the PARENT slot. Whether each one is
  // materialised, and how far its backfill goes, is decided in
  // linkAndBackfillNamespace.
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let parentPath = ENSv2NamePath.load(pathIndex.path);
    // Skip paths another registry's slot has since taken over: this slot's
    // subregistry says nothing about them any more.
    if (
      parentPath == null ||
      !parentPath.active ||
      !parentPath.slot.equals(parentSlot.id)
    ) {
      continue;
    }

    linkAndBackfillNamespace(
      childRegistry,
      parentSlot,
      parentPath,
      event.params.tokenId,
      event,
      0
    );
  }
}

// A name migrated from a locked ENSv1 name keeps its emancipated ENSv1
// subnames (legacyGovernance.ts rule 3) because the WrapperRegistry linked
// under it sends them to ENSv1. Any other subregistry, or none, does not:
// the name's remaining ENSv1 rows are cleared and it is marked like every
// other ENSv2 name. The migration links the WrapperRegistry in the
// transaction that registers the name, so this finds the registry's row
// (created by its ProxyDeployed) already there.
//
// A WrapperRegistry sends to ENSv1 the children of the one name it was
// deployed for (its `_node`), so the registry that counts is the one the
// migration linked: the first one, or that same address emitted again. A
// name that moves from one registry to another, WrapperRegistry or not, no
// longer has its own behind it. (Whether a registry is the name's own could
// also be read from its ParentUpdated, but a registry created in this block
// has its own events handled after everything else in the block, so the
// declaration is not there yet when the migration links it.)
function voidHoldoutsUnlessWrapperRegistry(
  parentSlot: ENSv2NameSlot,
  previousChildAddress: Bytes | null,
  event: SubregistryUpdated
): void {
  if (!isZeroAddress(event.params.subregistry)) {
    let sameRegistry = true;
    if (previousChildAddress) {
      sameRegistry = previousChildAddress.equals(event.params.subregistry);
    }
    let linked = ENSv2Registry.load(event.params.subregistry);
    if (sameRegistry && linked != null && linked.kind == "WRAPPER") {
      return;
    }
  }
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null || !path.slot.equals(parentSlot.id)) {
      continue;
    }
    if (path.legacySubtreeVoided || !path.domain) {
      continue;
    }
    voidLegacySubtree(path, false, event.block.timestamp);
  }
}

function labelhashOf(label: string): Bytes {
  return Bytes.fromByteArray(crypto.keccak256(ByteArray.fromUTF8(label)));
}

// The namespace that spells the registry's canonical name: its parent name
// is the one the registry itself declares (ParentUpdated: parent registry and
// label), and that name is in turn reached through a canonical namespace, up
// to the root. Only the registry's owner can make a namespace canonical, so
// it is the one namespace that cannot be claimed by linking the registry
// under other names. A registry that declares no parent has none.
function isCanonicalNamespace(
  registry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath
): boolean {
  let declaredParent = registry.canonicalParentRegistry;
  if (!declaredParent) {
    return false;
  }
  if (!declaredParent.equals(parentSlot.registry)) {
    return false;
  }
  let declaredLabel = registry.canonicalParentLabel;
  if (declaredLabel === null) {
    return false;
  }
  if (!labelhashOf(declaredLabel as string).equals(parentSlot.labelhash)) {
    return false;
  }
  let parentNamespace = ENSv2Namespace.load(parentPath.namespace);
  return parentNamespace != null && parentNamespace.canonical;
}

// The RootRegistry is the top of the tree and is never projected under a
// name; the ETHRegistry is projected under names of the RootRegistry only
// (eth). Anyone can point a name of theirs at either, for one transaction.
// Every .eth name registered from then on would be projected a second time
// under that name, and so would everything below it: each name in the
// registry of foo.eth would have a path under foo.eth and one under
// foo.<alias>.eth, would count twice against what may sit below foo.eth,
// and would take one of that registry's alias places, with eight such
// aliases nine times and all of them. The link is recorded and not
// projected, like any other refused link.
function isOwnPlaceOfCoreRegistry(
  registry: ENSv2Registry,
  parentSlot: ENSv2NameSlot
): boolean {
  if (registry.kind == "ROOT") {
    return false;
  }
  if (registry.kind == "ETH") {
    return parentSlot.registry.equals(getRootRegistryAddress());
  }
  return true;
}

// Decides whether the registry's names get paths under this active
// namespace, and records the decision on both (`materialized`; a refusal is
// not logged, one event can produce thousands). Refuses when
//   - the namespace sits MAX_BACKFILL_DEPTH registries below the link that
//     is being processed,
//   - the registry is already materialised under MAX_ALIAS_NAMESPACES
//     aliases (a canonical namespace is exempt) or under
//     MAX_MATERIALIZED_NAMESPACES namespaces in all,
//   - the registry has used up its lifetime projections of this kind,
//   - the parent name, or a name above it, has maxNamesBelowName units
//     below it already,
//   - the registry is the RootRegistry, or the ETHRegistry under a name
//     that is not the RootRegistry's (isOwnPlaceOfCoreRegistry).
// How many names the registry has does not matter here: backfillNamespace
// projects what the event's budget allows and queues the rest.
function tryMaterializeNamespace(
  namespace: ENSv2Namespace,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  depth: i32
): boolean {
  // Fresh load: a recursive backfill may have changed the registry.
  let registry = ENSv2Registry.load(namespace.registry);
  if (registry == null) {
    return false;
  }
  let canonical = isCanonicalNamespace(registry, parentSlot, parentPath);
  if (namespace.canonical != canonical) {
    namespace.canonical = canonical;
    namespace.save();
  }
  if (depth >= MAX_BACKFILL_DEPTH) {
    return false;
  }
  if (!isOwnPlaceOfCoreRegistry(registry, parentSlot)) {
    return false;
  }
  let materialized = registry.materializedNamespaces;
  if (materialized.length >= MAX_MATERIALIZED_NAMESPACES) {
    return false;
  }
  if (!canonical) {
    let aliases = 0;
    for (let i = 0; i < materialized.length; i++) {
      let other = ENSv2Namespace.load(materialized[i]);
      if (other != null && !other.canonical) {
        aliases++;
      }
    }
    if (aliases >= MAX_ALIAS_NAMESPACES) {
      return false;
    }
  }
  // The RootRegistry has no parent, and the ETHRegistry is linked once, at
  // deployment, before any .eth registration: neither is backfilled.
  if (registry.kind != "ROOT" && registry.kind != "ETH") {
    let used = canonical
      ? registry.canonicalMaterializations
      : registry.aliasMaterializations;
    if (used >= MAX_LIFETIME_MATERIALIZATIONS) {
      return false;
    }
    if (canonical) {
      registry.canonicalMaterializations = used + 1;
    } else {
      registry.aliasMaterializations = used + 1;
    }
  }
  // A projected subregistry is a unit below its parent name and the names
  // above (ENSv2SubtreeSize). Nothing has been saved yet, so a refusal here
  // uses up none of the registry's allowance.
  if (
    !reserveBelowAncestors(ancestorsFrom(namespace.parentPath), NAMESPACE_UNITS)
  ) {
    return false;
  }
  materialized.push(namespace.id);
  registry.materializedNamespaces = materialized;
  registry.save();
  // Its earlier paths, if it had any, were all retired when it was unlinked:
  // the backfill starts over.
  namespace.materialized = true;
  namespace.backfilled = false;
  namespace.backfillCursor = 0;
  namespace.save();
  return true;
}

// Creates or reactivates the namespace (childRegistry under parentPath).
// When it was not already materialised, names registered in childRegistry
// before the link have no path under it yet: materialise it if the limits
// allow, and backfill them. An active namespace that was refused earlier is
// tried again here, and one whose backfill is unfinished is taken further,
// which is what emitting the link again does.
function linkAndBackfillNamespace(
  childRegistry: ENSv2Registry,
  parentSlot: ENSv2NameSlot,
  parentPath: ENSv2NamePath,
  parentTokenId: BigInt,
  event: ethereum.Event,
  depth: i32
): void {
  let existing = ENSv2Namespace.load(
    namespaceId(childRegistry.id, parentPath.namehash)
  );
  let wasMaterialized = false;
  if (existing != null) {
    wasMaterialized = existing.active && existing.materialized;
  }
  let namespace = createOrReactivateNamespace(
    childRegistry,
    parentSlot,
    parentPath,
    parentTokenId,
    event
  );
  if (existing == null) {
    appendRegistryNamespaceIndex(childRegistry, namespace);
    appendPathNamespaceIndex(parentPath, namespace);
  }
  setActiveNamespace(parentPath, namespace, event.block);
  if (wasMaterialized) {
    if (!namespace.backfilled) {
      backfillNamespace(childRegistry.id, namespace, event, depth);
    }
    return;
  }
  if (tryMaterializeNamespace(namespace, parentSlot, parentPath, depth)) {
    backfillNamespace(childRegistry.id, namespace, event, depth);
  } else {
    // The link is live on chain but its names are not projected. An ENSv1
    // name below `parentPath` may now resolve through the registry behind
    // it, which the subgraph does not look into, so every ENSv1 row below
    // loses its resolver and address: no address rather than one that may be
    // wrong (legacyGovernance.ts rule 3). For a registered name this was
    // done when it was projected; it matters for a root-level name and for a
    // locked migration, whose emancipated subnames go too.
    voidLegacySubtree(parentPath, false, event.block.timestamp);
  }
}

// Names of the namespace's registry are missing under it (the event's budget
// ran out, or a name above is full). Until they are projected, the ENSv1
// rows below the parent name are treated as for a refused link.
function voidLegacyBelowNamespace(
  namespace: ENSv2Namespace,
  timestamp: BigInt
): void {
  let parentPathId = namespace.parentPath;
  if (!parentPathId) {
    return;
  }
  let parentPath = ENSv2NamePath.load(parentPathId);
  if (parentPath != null) {
    voidLegacySubtree(parentPath, false, timestamp);
  }
}

// ---------------------------------------------------------------------------
// Backfill queue
// ---------------------------------------------------------------------------

const BACKFILL_QUEUE_ID = Bytes.fromHexString("0x00");

function backfillTaskId(position: i32): Bytes {
  return Bytes.fromByteArray(i32ToBytes(position));
}

// Puts the namespace at the back of the queue. Sets backfillQueued on the
// caller's object, which the caller saves.
function enqueueBackfill(namespace: ENSv2Namespace): void {
  if (namespace.backfillQueued) {
    return;
  }
  let queue = ENSv2BackfillQueue.load(BACKFILL_QUEUE_ID);
  if (queue == null) {
    queue = new ENSv2BackfillQueue(BACKFILL_QUEUE_ID);
    queue.head = 0;
    queue.tail = 0;
  }
  let task = new ENSv2BackfillTask(backfillTaskId(queue.tail));
  task.namespace = namespace.id;
  task.save();
  queue.tail = queue.tail + 1;
  queue.save();
  namespace.backfillQueued = true;
}

// Takes up the backfills an earlier event left unfinished, with whatever is
// left of this event's budget. Called by the registry handlers that run
// often (ensv2Registry.ts), so a registry with more names than one event
// projects is completed by the events that follow, whoever emits them. One
// load when the queue is empty.
export function continueQueuedBackfills(event: ethereum.Event): void {
  for (let i = 0; i < MAX_BACKFILL_TASKS_PER_EVENT; i++) {
    if (backfillSlotBudget <= 0) {
      return;
    }
    let queue = ENSv2BackfillQueue.load(BACKFILL_QUEUE_ID);
    if (queue == null || queue.head >= queue.tail) {
      return;
    }
    let taskId = backfillTaskId(queue.head);
    queue.head = queue.head + 1;
    queue.save();
    let task = ENSv2BackfillTask.load(taskId);
    if (task == null) {
      continue;
    }
    store.remove("ENSv2BackfillTask", taskId.toHexString());
    let namespace = ENSv2Namespace.load(task.namespace);
    if (namespace == null) {
      continue;
    }
    namespace.backfillQueued = false;
    namespace.save();
    // Unlinked or finished since it was queued: nothing to do. One that is
    // still unfinished after this goes to the back of the queue again.
    if (!namespace.active || !namespace.materialized || namespace.backfilled) {
      continue;
    }
    backfillNamespace(namespace.registry, namespace, event, 0);
  }
}

// Projects the names already REGISTERED in the namespace's registry, from
// where an earlier event left off (namespace.backfillCursor), and follows
// each one's subregistry. Same projection as a registration, except that the
// slot's resolver is attached here (at registration time the registry emits
// ResolverUpdated afterwards instead). The caller has already admitted the
// namespace (tryMaterializeNamespace).
//
// Stops when the event's budget is used up, and then queues the namespace,
// or when a name above is full (maxNamesBelowName), and then leaves it for
// the link to be emitted again. Either way `backfilled` stays false and the
// ENSv1 rows below the parent name are cleared.
function backfillNamespace(
  registryId: Bytes,
  namespace: ENSv2Namespace,
  event: ethereum.Event,
  depth: i32
): void {
  // Fresh load: the caller's snapshot may predate namespace index appends.
  let registry = ENSv2Registry.load(registryId);
  if (registry == null) {
    return;
  }
  // The RootRegistry has no parent, and the ETHRegistry is linked once, at
  // deployment, before any .eth registration.
  if (registry.kind == "ROOT" || registry.kind == "ETH") {
    namespace.backfilled = true;
    namespace.save();
    return;
  }
  let slotCount = registry.slotCount;
  let cursor = namespace.backfillCursor;
  let chain = ancestorsFrom(namespace.parentPath);
  let slotCost = backfillSlotCost(chain);
  let full = false;
  while (cursor < slotCount) {
    if (backfillSlotBudget <= 0) {
      break;
    }
    backfillSlotBudget -= slotCost;
    let index = ENSv2RegistrySlotIndex.load(registrySlotIndexId(registryId, cursor));
    if (index != null) {
      let slot = ENSv2NameSlot.load(index.slot);
      if (slot != null && slot.status == "REGISTERED") {
        if (!backfillSlot(registry, namespace, slot, event, depth, chain)) {
          full = true;
          break;
        }
      }
    }
    cursor++;
  }

  // Fresh load: the path index appends above saved the namespace.
  let current = ENSv2Namespace.load(namespace.id);
  if (current == null) {
    return;
  }
  current.backfillCursor = cursor;
  current.backfilled = cursor >= slotCount;
  if (!current.backfilled && !full) {
    enqueueBackfill(current);
  }
  current.save();
  namespace.backfillCursor = current.backfillCursor;
  namespace.backfilled = current.backfilled;
  namespace.backfillQueued = current.backfillQueued;
  if (!current.backfilled) {
    voidLegacyBelowNamespace(current, event.block.timestamp);
  }
}

// One slot of a backfill. Returns false when the name could not be projected
// because a name above it is full.
function backfillSlot(
  registry: ENSv2Registry,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: ethereum.Event,
  depth: i32,
  chain: AncestorChain
): boolean {
  // Registered after the link, while the backfill was unfinished: projected
  // then, with its resolver and subregistry following from its own events.
  let existing = ENSv2NamePath.load(
    pathNamehash(namespace.baseNamehash, slot.labelhash)
  );
  if (existing != null && existing.active && existing.slot.equals(slot.id)) {
    return true;
  }
  // A name that is too long or too deep is never projected: the backfill
  // goes on past it, and the ENSv1 row it may have had is cleared with the
  // rest below the parent name.
  if (existing == null && !fitsNameLimits(chain, slot)) {
    voidLegacyBelowNamespace(namespace, event.block.timestamp);
    return true;
  }
  if (isHeldByAnotherSlot(existing, slot)) {
    return true;
  }
  let path = materializePathInNamespace(
    registry,
    namespace,
    slot,
    event,
    slot.migratedFromV1,
    chain
  );
  if (path == null) {
    return false;
  }
  if (path.domain != null) {
    let resolverAddress = slot.resolverAddress;
    if (resolverAddress) {
      attachDomainResolver(Address.fromBytes(resolverAddress), path.namehash, true);
    }
  }
  let subregistry = slot.subregistry;
  if (subregistry) {
    backfillSlotBudget -= backfillLinkCost(chain);
    let childRegistry = getOrCreateRegistry(
      subregistry,
      Address.fromBytes(subregistry),
      event.block
    );
    linkAndBackfillNamespace(
      childRegistry,
      slot,
      path,
      currentTokenIdOf(slot),
      event,
      depth + 1
    );
  }
  return true;
}

function currentTokenIdOf(slot: ENSv2NameSlot): BigInt {
  let tokenEntityId = slot.currentToken;
  if (tokenEntityId) {
    let token = ENSv2Token.load(tokenEntityId);
    if (token != null) {
      return token.tokenId;
    }
  }
  return slot.slotId;
}

// The other bounded loop (see file header): one path per namespace the
// REGISTERING slot's own registry is materialised under, created or
// reactivated. At most MAX_MATERIALIZED_NAMESPACES of them. Called from
// ensv2Registry.ts::handleLabelRegistered after the slot is saved.
export function materializePathsForSlot(
  registry: ENSv2Registry,
  slot: ENSv2NameSlot,
  event: ethereum.Event,
  isV1Migration: boolean
): void {
  let materialized = registry.materializedNamespaces;
  for (let i = 0; i < materialized.length; i++) {
    let namespace = ENSv2Namespace.load(materialized[i]);
    if (namespace == null || !namespace.active || !namespace.materialized) {
      continue;
    }
    let chain = ancestorsFrom(namespace.parentPath);
    let existing = ENSv2NamePath.load(
      pathNamehash(namespace.baseNamehash, slot.labelhash)
    );
    if (existing == null && !fitsNameLimits(chain, slot)) {
      // Too long or too deep: never projected, so nothing for a later
      // backfill to pick up. The ENSv1 row it may have had is cleared with
      // the rest below the parent name.
      voidLegacyBelowNamespace(namespace, event.block.timestamp);
      continue;
    }
    if (isHeldByAnotherSlot(existing, slot)) {
      continue;
    }
    let path = materializePathInNamespace(
      registry,
      namespace,
      slot,
      event,
      isV1Migration,
      chain
    );
    if (path == null) {
      // A name above is full. The namespace now misses this name: a backfill
      // resumed by emitting the link again starts at its slot, or earlier if
      // one was already missing.
      let current = ENSv2Namespace.load(namespace.id);
      if (current != null) {
        if (current.backfilled || slot.registryIndex < current.backfillCursor) {
          current.backfillCursor = slot.registryIndex;
        }
        current.backfilled = false;
        current.save();
        voidLegacyBelowNamespace(current, event.block.timestamp);
      }
    }
  }
}

// A live name belongs to the slot that has it. A path changes hands when a
// name switches to another subregistry, and by then the old registry's
// namespace has been retired and its paths with it, so a slot that claims a
// path which is still active under another slot is a second token carrying
// the same labelhash, which only a contract that emits whatever it likes can
// produce. It is left without the name: taking the path would bring
// everything counted below it onto a slot that never passed the limit for
// it.
function isHeldByAnotherSlot(
  path: ENSv2NamePath | null,
  slot: ENSv2NameSlot
): boolean {
  return path != null && path.active && !path.slot.equals(slot.id);
}

// Materialises (or rebinds and reactivates) the slot's path under one
// namespace and projects it. Returns null, and writes nothing, when the path
// would become active under a name that already has maxNamesBelowName units
// below it, or when a new path would not fit the name limits (callers check
// fitsNameLimits first to tell the two apart). `chain` is the namespace's
// ancestors (ancestorsFrom(namespace.parentPath)).
function materializePathInNamespace(
  registry: ENSv2Registry,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: ethereum.Event,
  isV1Migration: boolean,
  chain: AncestorChain
): ENSv2NamePath | null {
  let pathId = pathNamehash(namespace.baseNamehash, slot.labelhash);
  let path = ENSv2NamePath.load(pathId);
  if (path == null && !fitsNameLimits(chain, slot)) {
    return null;
  }
  if (isHeldByAnotherSlot(path, slot)) {
    return null;
  }
  if (path == null || !path.active) {
    if (!reserveBelowAncestors(chain, 1)) {
      return null;
    }
  }
  if (path == null) {
    path = materializeNamePath(pathId, namespace, slot, event);
    addToSlotPaths(slot, path);
    addToNamespacePaths(namespace, path);
    path.save();
  } else {
    // A path id is the namehash, so a parent that switched its subregistry
    // to a new registry hands the same path to the new registry's slot.
    // Rebind it, or the new slot's transfers, renewals and unregistration
    // never reach the projected Domain.
    if (!path.slot.equals(slot.id)) {
      // Whatever is still counted below the path moves with it. Nothing, as
      // a rule: the path is inactive (isHeldByAnotherSlot), so what was
      // below it was retired.
      let below = ENSv2SubtreeSize.load(path.id);
      if (below != null && below.paths > 0) {
        addToSubtreeSize(path.slot, -below.paths);
        addToSubtreeSize(slot.id, below.paths);
      }
      // It leaves the old namespace's list, if it is still in it, and the
      // old slot's.
      removeFromNamespacePaths(path);
      removeFromSlotPaths(path);
      path.slot = slot.id;
      path.registry = slot.registry;
      path.namespace = namespace.id;
      addToSlotPaths(slot, path);
    }
    // A path deactivated by LabelUnregistered or a namespace unlink comes
    // back: its Domain row (if any) is reused by the projection below, which
    // counts it in its parent again. Rebound or reactivated, it is not in
    // this namespace's list yet.
    if (path.namespaceIndex < 0) {
      addToNamespacePaths(namespace, path);
    }
    path.active = true;
    path.updatedAt = event.block.timestamp;
    path.updatedAtBlock = event.block.number;
    path.save();
  }

  // RootRegistry paths are native ENSv2 metadata for the root namespace,
  // not compatibility projections. Their IDs are ordinary ENSv1 namehashes
  // (for example, <eth> and <reverse>), so projecting them would overwrite
  // the existing ENSv1 Domain rows for those nodes.
  if (registry.kind != "ROOT") {
    projectPathToDomain(path, slot, event, isV1Migration);
  }
  return path;
}

function materializeNamePath(
  pathId: Bytes,
  namespace: ENSv2Namespace,
  slot: ENSv2NameSlot,
  event: ethereum.Event
): ENSv2NamePath {
  let path = new ENSv2NamePath(pathId);
  path.slot = slot.id;
  path.registry = slot.registry;
  path.namespace = namespace.id;

  let parentPathId = namespace.parentPath;
  let depth = 0;
  let name: string | null = null;
  let label: string | null = null;
  if (checkValidLabel(slot.label)) {
    label = slot.label;
  } else {
    label = "[" + slot.labelhash.toHexString().slice(2) + "]";
  }
  path.label = label;

  if (parentPathId) {
    path.parent = parentPathId;
    let parentPath = ENSv2NamePath.load(parentPathId);
    if (parentPath != null) {
      depth = parentPath.depth + 1;
      if (parentPath.name !== null && label !== null) {
        name = (label as string) + "." + (parentPath.name as string);
      }
    }
  } else {
    // Root-level namespace (e.g. the "eth" registration under RootRegistry).
    depth = 0;
    name = label;
  }

  path.name = name;
  path.labelhash = slot.labelhash;
  path.namehash = pathNamehash(namespace.baseNamehash, slot.labelhash);
  path.depth = depth;
  path.active = true;
  path.namespaceCount = 0;
  path.namespaceIndex = -1;
  path.slotIndex = -1;
  path.legacySubtreeVoided = false;
  path.createdAt = event.block.timestamp;
  path.updatedAt = event.block.timestamp;
  path.createdAtBlock = event.block.number;
  path.updatedAtBlock = event.block.number;
  path.save();
  return path;
}

export function handleResolverUpdated(event: ResolverUpdated): void {
  let registryId = event.address;
  let slotId = toSlotId(event.params.tokenId);
  let slot = ENSv2NameSlot.load(nameSlotId(registryId, slotId));
  if (slot == null) {
    log.warning("ResolverUpdated for unknown slot {} on registry {}", [
      slotId.toString(),
      registryId.toHexString(),
    ]);
    return;
  }

  if (isZeroAddress(event.params.resolver)) {
    slot.resolverAddress = null;
    slot.resolver = null;
  } else {
    slot.resolverAddress = event.params.resolver;
    // Share resolver creation with the resolver event mapping.
    let resolverEntity = getOrCreateResolver(event.params.resolver);
    slot.resolver = resolverEntity.id;
  }
  slot.updatedAt = event.block.timestamp;
  slot.updatedAtBlock = event.block.number;
  slot.save();

  // Keep the existing compatibility projection synchronized with the native
  // slot.
  for (let i = 0; i < slot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(slot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let path = ENSv2NamePath.load(pathIndex.path);
    if (path == null) {
      continue;
    }
    if (!path.active || !path.slot.equals(slot.id)) {
      continue;
    }
    if (path.domain == null) {
      continue;
    }
    if (isZeroAddress(event.params.resolver)) {
      clearDomainResolver(path.namehash);
    } else {
      attachDomainResolver(event.params.resolver, path.namehash, true);
    }
  }

  let history = new ENSv2ResolverUpdate(createEventID(event));
  history.slot = slot.id;
  history.blockNumber = event.block.number;
  history.transactionID = event.transaction.hash;
  history.logIndex = event.logIndex;
  history.resolverAddress = isZeroAddress(event.params.resolver)
    ? null
    : event.params.resolver;
  history.sender = event.params.sender;
  history.save();
}

// The namespaces in `changed` became canonical, or stopped being so. Whether
// a namespace is canonical depends on the one above it (isCanonicalNamespace),
// so the registries linked below their names are looked at again, and so on
// down for as long as the answer changes. A registry that declared its
// parent before the registry above declared its own would otherwise go on
// counting as an alias: it would use one of its alias places, and be refused
// if they were taken. A namespace that turns out canonical and was refused
// as an alias is materialised here.
//
// Walks the active names of each namespace it descends into, and stops after
// maxCanonicalReconcilePaths of them. Below one name that is as much as can
// be there (maxNamesBelowName), but the names of the ETHRegistry are not
// limited, and its own declaration changing must not mean a walk over every
// .eth name. Where it stops, the registries further on keep the flag they
// had until they are linked or declare again: they count as aliases when
// they could be canonical, or the reverse, which decides only whether a
// registry is projected, never what a projected name shows.
function reconcileCanonicalBelow(changed: Array<Bytes>, event: ethereum.Event): void {
  let pending = changed;
  let visited = 0;
  while (pending.length > 0) {
    let namespace = ENSv2Namespace.load(pending.pop());
    if (namespace == null || !namespace.active || !namespace.materialized) {
      continue;
    }
    for (let i = 0; i < namespace.pathCount; i++) {
      if (visited >= maxCanonicalReconcilePaths) {
        log.warning(
          "reconcileCanonicalBelow: more than {} names below the declaration at {}, the rest keep their canonical flag",
          [maxCanonicalReconcilePaths.toString(), namespace.id.toHexString()]
        );
        return;
      }
      visited++;
      let entry = ENSv2NamespacePathIndex.load(
        namespacePathIndexId(namespace.id, i)
      );
      if (entry == null) {
        continue;
      }
      let path = ENSv2NamePath.load(entry.path);
      if (path == null || !path.active) {
        continue;
      }
      let belowId = path.activeNamespace;
      if (!belowId) {
        continue;
      }
      let below = ENSv2Namespace.load(belowId);
      if (below == null || !below.active) {
        continue;
      }
      let belowRegistry = ENSv2Registry.load(below.registry);
      let slot = ENSv2NameSlot.load(path.slot);
      if (belowRegistry == null || slot == null) {
        continue;
      }
      let canonical = isCanonicalNamespace(belowRegistry, slot, path);
      if (below.canonical == canonical) {
        continue;
      }
      if (below.materialized) {
        below.canonical = canonical;
        below.save();
        pending.push(below.id);
      } else if (canonical) {
        // tryMaterializeNamespace records the flag; the backfill evaluates
        // what is below it as it goes.
        if (tryMaterializeNamespace(below, slot, path, 0)) {
          backfillNamespace(below.registry, below, event, 0);
        }
      } else {
        below.canonical = canonical;
        below.save();
      }
    }
  }
}

// The registry changed the parent it declares, so which of its namespaces is
// canonical may have changed with it. Re-evaluates the materialised ones
// (at most MAX_MATERIALIZED_NAMESPACES), then materialises the canonical
// namespace if the link already exists but was refused: a link processed
// before this declaration only counted as an alias.
function reconcileCanonicalNamespaces(
  registryId: Bytes,
  event: ethereum.Event
): void {
  let registry = ENSv2Registry.load(registryId);
  if (registry == null) {
    return;
  }
  let materialized = registry.materializedNamespaces;
  let changed = new Array<Bytes>();
  for (let i = 0; i < materialized.length; i++) {
    let namespace = ENSv2Namespace.load(materialized[i]);
    if (namespace == null) {
      continue;
    }
    let canonical = false;
    let parentSlotId = namespace.parentSlot;
    let parentPathId = namespace.parentPath;
    if (parentSlotId && parentPathId) {
      let parentSlot = ENSv2NameSlot.load(parentSlotId);
      let parentPath = ENSv2NamePath.load(parentPathId);
      if (parentSlot != null && parentPath != null) {
        canonical = isCanonicalNamespace(registry, parentSlot, parentPath);
      }
    }
    if (namespace.canonical != canonical) {
      namespace.canonical = canonical;
      namespace.save();
      changed.push(namespace.id);
    }
  }
  reconcileCanonicalBelow(changed, event);

  let declaredParent = registry.canonicalParentRegistry;
  let declaredLabel = registry.canonicalParentLabel;
  if (!declaredParent || declaredLabel === null) {
    return;
  }
  let parentSlot = ENSv2NameSlot.load(
    slotEntityIdForLabelhash(declaredParent, labelhashOf(declaredLabel as string))
  );
  if (parentSlot == null) {
    return;
  }
  let linked = parentSlot.subregistry;
  if (!linked || !linked.equals(registryId)) {
    return;
  }
  for (let i = 0; i < parentSlot.pathCount; i++) {
    let pathIndex = ENSv2SlotPathIndex.load(slotPathIndexId(parentSlot.id, i));
    if (pathIndex == null) {
      continue;
    }
    let parentPath = ENSv2NamePath.load(pathIndex.path);
    if (
      parentPath == null ||
      !parentPath.active ||
      !parentPath.slot.equals(parentSlot.id)
    ) {
      continue;
    }
    let namespace = ENSv2Namespace.load(
      namespaceId(registryId, parentPath.namehash)
    );
    if (namespace == null || !namespace.active || namespace.materialized) {
      continue;
    }
    // Only the canonical namespace: a refused alias stays refused until its
    // own link is emitted again.
    let current = ENSv2Registry.load(registryId);
    if (
      current == null ||
      !isCanonicalNamespace(current, parentSlot, parentPath)
    ) {
      continue;
    }
    if (tryMaterializeNamespace(namespace, parentSlot, parentPath, 0)) {
      backfillNamespace(registryId, namespace, event, 0);
    }
  }
}

// Stores the parent a registry declares (getParent()). Dynamically linked
// registries may legitimately return (0x0, ""). The declaration decides
// which namespace of the registry is canonical (isCanonicalNamespace).
export function handleParentUpdated(event: ParentUpdated): void {
  resetBackfillBudget();
  let registryId = event.address;
  let registry = ENSv2Registry.load(registryId);
  if (registry == null) {
    // The wrapper normally bootstraps this row first. Keep a guard so
    // unexpected event order does not terminate indexing.
    log.warning("ParentUpdated for unknown registry {}", [
      registryId.toHexString(),
    ]);
    return;
  }

  if (isZeroAddress(event.params.parent)) {
    registry.canonicalParentRegistry = null;
    // Keep the registry and label fields consistent when the parent is clear.
    registry.canonicalParentLabel = null;
  } else {
    let parentRegistry = getOrCreateRegistry(
      event.params.parent,
      event.params.parent,
      event.block
    );
    registry.canonicalParentRegistry = parentRegistry.id;
    if (checkValidLabel(event.params.label)) {
      registry.canonicalParentLabel = event.params.label;
    } else {
      // Do not retain the previous parent's label beside a new parent or
      // persist an invalid replacement label.
      registry.canonicalParentLabel = null;
    }
  }
  registry.updatedAtBlock = event.block.number;
  registry.save();

  reconcileCanonicalNamespaces(registryId, event);
}
